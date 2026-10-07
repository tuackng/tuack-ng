use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;

use crate::command::string_to_command;
use crate::prelude::*;
use crate::process::ProcessSupervisor;
use tuack_lib::data::Reader;
use tuack_lib::utils::compiler::{IoMode, RunResult, RunSpec, Runner, RunnerManifest};

/// C++ 运行器的 [`Runner`] 实现：用 `g++` 编译源文件后执行；`set_interactive` 后额外
/// 链接 grader.cpp，并把交互库头文件复制为 `{program_name}.h` 供其包含
pub struct CppRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    interactive: bool,
    grader_path: Option<PathBuf>,
    header_path: Option<PathBuf>,
}

impl CppRunner {
    /// 创建临时目录，并按源文件扩展名从 `compile_args` 取编译选项。
    ///
    /// # Errors
    ///
    /// 源文件无扩展名，或 `compile_args` 未登记该扩展名时返回 `Err`。
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &IndexMap<String, String>,
        program_name: impl Into<String>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-runner-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(CppRunner {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .context("没有该语言编译选项")?
                .to_string(),
            program_name,
            interactive: false,
            grader_path: None,
            header_path: None,
        })
    }

    fn get_compile_command(&self) -> Result<StdCommand> {
        let exe_path = self
            .tmp_dir
            .path()
            .join(format!(
                "{}{}",
                self.program_name,
                std::env::consts::EXE_SUFFIX
            ))
            .display()
            .to_string()
            .replace("\\", "\\\\")
            .replace(" ", "\\ ");

        let source_path = self
            .tmp_dir
            .path()
            .join(&self.program_name)
            .with_extension(self.source.extension().unwrap())
            .display()
            .to_string()
            .replace("\\", "\\\\")
            .replace(" ", "\\ ");

        let mut cmd_str = format!("g++ -o {} {} {}", exe_path, self.compile_args, source_path);

        if self.interactive {
            let grader_path = self
                .tmp_dir
                .path()
                .join("grader.cpp")
                .display()
                .to_string()
                .replace("\\", "\\\\")
                .replace(" ", "\\ ");
            cmd_str = format!("{} {}", cmd_str, grader_path);
        }

        string_to_command(&cmd_str)
    }
}

impl Runner for CppRunner {
    fn manifest(&self) -> RunnerManifest {
        RunnerManifest { interactive: true }
    }

    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(&self.tmp_dir)?;
        }

        let mut cmd = self.get_compile_command()?;

        let source_target_path = self
            .tmp_dir
            .path()
            .join(&self.program_name)
            .with_extension(self.source.extension().unwrap());

        fs::copy(&self.source, &source_target_path)?;

        if self.interactive {
            let grader_target_path = self.tmp_dir.path().join("grader.cpp");
            fs::copy(self.grader_path.as_ref().unwrap(), &grader_target_path)?;
            let header_target_path = self.tmp_dir.path().join(format!("{}.h", self.program_name));
            fs::copy(self.header_path.as_ref().unwrap(), &header_target_path)?;
        }

        let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
        if !output.status.success() {
            bail!("编译错误：{}", String::from_utf8_lossy(&output.stderr));
        }

        fs::remove_file(&source_target_path)?;

        Ok(())
    }

    fn set_interactive(&mut self, grader_file: &Path, header_file: &Path) -> Result<()> {
        self.interactive = true;
        self.grader_path = Some(grader_file.to_owned());
        self.header_path = Some(header_file.to_owned());
        Ok(())
    }

    fn execute(&mut self, spec: RunSpec) -> Result<RunResult> {
        let RunSpec {
            limits,
            io_mode,
            mut input,
        } = spec;

        let program_path = self.tmp_dir.path().join(format!(
            "{}{}",
            self.program_name,
            std::env::consts::EXE_SUFFIX
        ));
        if !program_path.exists() {
            bail!("可执行文件不存在：{}", program_path.display());
        }

        let mut cmd = StdCommand::new(&program_path);
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

        // 输出文件不存在视为无输出；其余 IO 错误上抛
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
