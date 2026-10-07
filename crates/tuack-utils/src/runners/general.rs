use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;

use crate::command::string_to_command;
use crate::prelude::*;
use crate::process::ProcessSupervisor;
use strfmt::strfmt;
use tuack_config::lang::Language;
use tuack_lib::data::Reader;
use tuack_lib::utils::compiler::{IoMode, RunResult, RunSpec, Runner, RunnerManifest};

/// 通用运行器的 [`Runner`] 实现：按 [`Language`] 配置模板渲染编译与运行命令，再在资源限制
/// 下执行；无编译器的语言直接拷贝源文件
pub struct GeneralRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    language: Language,
    program_name: String,
}

impl GeneralRunner {
    /// 创建临时目录，并按源文件扩展名查编译选项与语言配置。
    ///
    /// # Errors
    ///
    /// 源文件无扩展名，或 `compile_args`/`languages` 未登记该扩展名时返回 `Err`。
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &IndexMap<String, String>,
        program_name: impl Into<String>,
        languages: &IndexMap<String, Language>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-runner-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(GeneralRunner {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .context("没有该语言编译选项")?
                .to_string(),
            language: languages.get(&ext).context("未知格式文件")?.to_owned(),
            program_name,
        })
    }

    fn get_compile_command(&self) -> Result<Option<StdCommand>> {
        if let Some(ref compile) = self.language.compiler {
            let compile_cmd = strfmt(
                &compile.run,
                &HashMap::from([
                    ("executable".to_string(), compile.executable.clone()),
                    (
                        "output_path".to_string(),
                        self.tmp_dir
                            .path()
                            .to_string_lossy()
                            .to_string()
                            .replace("\\", "\\\\")
                            .replace(" ", "\\ "),
                    ),
                    (
                        "program_name".to_string(),
                        self.program_name
                            .clone()
                            .replace("\\", "\\\\")
                            .replace(" ", "\\ "),
                    ),
                    ("args".to_string(), self.compile_args.clone()),
                    (
                        "input_path".to_string(),
                        self.tmp_dir
                            .path()
                            .join(&self.program_name)
                            .with_extension(self.source.extension().unwrap())
                            .to_string_lossy()
                            .to_string()
                            .replace("\\", "\\\\")
                            .replace(" ", "\\ "),
                    ),
                    (
                        "exe_suffix".to_string(),
                        std::env::consts::EXE_SUFFIX.to_string(),
                    ),
                ]),
            )?;
            Ok(Some(string_to_command(compile_cmd.as_str())?))
        } else {
            Ok(None)
        }
    }

    fn get_run_base_command(&self) -> Result<StdCommand> {
        if let Some(ref runner) = self.language.runner {
            let run_cmd = strfmt(
                &runner.run,
                &HashMap::from([
                    ("executable".to_string(), runner.executable.clone()),
                    (
                        "input_path".to_string(),
                        self.tmp_dir
                            .path()
                            .to_string_lossy()
                            .to_string()
                            .replace("\\", "\\\\")
                            .replace(" ", "\\ "),
                    ),
                    (
                        "program_name".to_string(),
                        self.program_name
                            .clone()
                            .replace("\\", "\\\\")
                            .replace(" ", "\\ "),
                    ),
                    (
                        "exe_suffix".to_string(),
                        std::env::consts::EXE_SUFFIX.to_string(),
                    ),
                ]),
            )?;
            Ok(string_to_command(run_cmd.as_str())?)
        } else {
            let program_path = self.tmp_dir.path().join(format!(
                "{}{}",
                self.program_name,
                std::env::consts::EXE_SUFFIX
            ));
            if !program_path.exists() {
                bail!("可执行文件不存在：{}", program_path.display());
            }
            Ok(StdCommand::new(program_path))
        }
    }
}

impl Runner for GeneralRunner {
    fn manifest(&self) -> RunnerManifest {
        RunnerManifest { interactive: false }
    }

    /// 语言无编译器配置时只把源文件拷贝为 `{program_name}.{扩展名}`。
    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(&self.tmp_dir)?;
        }

        if let Some(mut cmd) = self.get_compile_command()? {
            let target_path = self
                .tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension(self.source.extension().unwrap());

            fs::copy(&self.source, &target_path)?;

            debug!("{cmd:#?}");

            let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
            debug!("??");
            if !output.status.success() {
                bail!("编译错误：{}", String::from_utf8_lossy(&output.stderr));
            }

            fs::remove_file(&target_path)?;
        } else {
            let target_path = self
                .tmp_dir
                .path()
                .join(self.program_name.clone())
                .with_extension(self.source.extension().unwrap());
            fs::copy(&self.source, target_path)?;
        }

        Ok(())
    }

    /// 通用运行器不支持交互：模板无法链接交互库
    fn set_interactive(&mut self, _grader_file: &Path, _header_file: &Path) -> Result<()> {
        unreachable!("通用运行器不支持交互");
    }

    /// 语言未配运行模板时直接执行 `{program_name}`；运行命令模板渲染或解析失败时返回 `Err`。
    fn execute(&mut self, spec: RunSpec) -> Result<RunResult> {
        let RunSpec {
            limits,
            io_mode,
            mut input,
        } = spec;

        let mut cmd = self.get_run_base_command()?;
        cmd.current_dir(&self.tmp_dir);

        // IO 模式决定数据读写方式：Stdio 把输入写入文件后接入 stdin、标准输出落文件；
        // File 由程序自行读写 input_name/output_name，标准流置空
        match &io_mode {
            IoMode::Stdio => {
                let stdin_path = self.tmp_dir.path().join("pipe_stdin");
                let stdout_path = self.tmp_dir.path().join("pipe_stdout");
                {
                    let mut stdin_file = std::fs::File::create(&stdin_path)?;
                    std::io::copy(&mut input, &mut stdin_file)?;
                }
                let stdin_handle = std::fs::File::open(&stdin_path)?;
                let stdout_handle = std::fs::File::create(&stdout_path)?;
                cmd.stdin(Stdio::from(stdin_handle));
                cmd.stdout(Stdio::from(stdout_handle));
            }
            IoMode::File { input_name, .. } => {
                let input_path = self.tmp_dir.path().join(input_name);
                {
                    let mut input_file = std::fs::File::create(&input_path)?;
                    std::io::copy(&mut input, &mut input_file)?;
                }
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
            }
        }

        let stderr_path = self.tmp_dir.path().join("pipe_stderr");
        let stderr_file = std::fs::File::create(&stderr_path)?;
        cmd.stderr(Stdio::from(stderr_file));

        let (status, time, memory) = ProcessSupervisor::new(limits).supervise_blocking(cmd)?;

        let stderr = std::fs::read(stderr_path)?;

        // 输出流：只有输出文件不存在时视为无输出；权限等其他错误传播
        let output: Option<Box<dyn Reader>> = {
            let output_path = match &io_mode {
                IoMode::Stdio => self.tmp_dir.path().join("pipe_stdout"),
                IoMode::File { output_name, .. } => self.tmp_dir.path().join(output_name),
            };
            if !output_path.exists() {
                None
            } else {
                Some(Box::new(std::fs::File::open(&output_path)?))
            }
        };

        Ok(RunResult {
            status,
            time,
            memory,
            output,
            stderr,
        })
    }

    fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}
