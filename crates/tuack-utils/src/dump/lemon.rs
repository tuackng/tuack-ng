//! Lemon 导出器。
//!
//! 输出为 Lemon 的竞赛目录：
//!
//! ```text
//! <场次名>.cdf                      竞赛配置
//! data/<题目名>/<题目名><编号>.in   输入
//! data/<题目名>/<题目名><编号>.ans  答案
//! data/<题目名>/chk[.exe]           自定义 SPJ（由本导出器编译生成）
//! ```
//!
//! `.cdf` 中的测试点路径相对 `data/`。评分策略与编译选项的限制见 [`LemonDumper`]。
use serde_json::{Map, Value, json};
use std::process::Command;

use crate::prelude::*;
use crate::utils::KeepAliveReader;
use tempfile::TempDir;
use tuack_lib::dump::{Dumper, ScorePolicy};
use tuack_lib::ren::ProblemType;
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LemonCase {
    full_score: u32,
    time_limit: u32,
    memory_limit: u32,
    input_files: Vec<String>,
    output_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LemonProblem {
    answer_file_extension: String,
    comparison_mode: u32,
    special_judge: PathBuf,
    diff_arguments: String,
    input_file_name: String,
    output_file_name: String,
    problem_title: String,
    task_type: u32,
    compiler_configuration: Map<String, Value>,
    test_cases: Vec<LemonCase>,
}

const COMPILER_MAP: &[(&str, &str)] = &[
    ("cpp", "g++"),
    ("c", "gcc"),
    ("pas", "fpc"),
    ("py", "python"),
    ("java", "javac"),
];

/// 取语言名对应的 Lemon 编译器名（如 `cpp` -> `g++`）
fn compiler_for_lang(lang: &str) -> Result<&'static str> {
    COMPILER_MAP
        .iter()
        .find(|(k, _)| *k == lang)
        .map(|(_, v)| *v)
        .ok_or_else(|| anyhow!("不支持的语言：{lang}"))
}

fn case_rel_path(prob_name: &str, case_id: u32, ext: &str) -> String {
    format!("{prob_name}/{prob_name}{case_id}.{ext}")
}

/// Lemon 导出器
///
/// 仅支持求和与取最小两种评分策略；编译选项只能按 Lemon 的 default 导出，需用户手动调整。
pub struct LemonDumper {
    tmp: Arc<TempDir>,
}

impl LemonDumper {
    pub fn new(tmp: Arc<TempDir>) -> Self {
        Self { tmp }
    }
}

impl Dumper for LemonDumper {
    /// 子任务使用 [`ScorePolicy::Max`]、题目为 [`ProblemType::Interactive`]、参赛语言无对应
    /// 编译器，或自定义 SPJ 编译失败时返回 `Err`；其余失败条件见 [`Dumper::dump`]。
    fn dump(
        &self,
        doc: &tuack_lib::dump::DumpDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(Vec<OutputFile>, Vec<String>)> {
        let mut files = Vec::new();
        let mut prob_jsons: Vec<Value> = Vec::new();
        let mut warnings = Vec::new();

        for prob in &doc.problems {
            for case in &prob.data {
                files.push(OutputFile::File {
                    path: PathBuf::from(format!(
                        "data/{}/{}{}.in",
                        prob.meta.name, prob.meta.name, case.id
                    )),
                    bytes: assets.load(prob.idx, &case.input)?,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!(
                        "data/{}/{}{}.ans",
                        prob.meta.name, prob.meta.name, case.id
                    )),
                    bytes: assets.load(prob.idx, &case.output)?,
                });
            }

            let mut cases: Vec<LemonCase> = Vec::new();
            let time_limit = (prob.meta.time_limit.as_secs_f64() * 1000.0) as u32;
            let memory_limit = prob.meta.memory_limit.as_mib() as u32;

            for task in prob.subtasks.values() {
                let input_files: Vec<String> = task
                    .items
                    .iter()
                    .map(|&idx| case_rel_path(&prob.meta.name, prob.data[idx].id, "in"))
                    .collect();
                let output_files: Vec<String> = task
                    .items
                    .iter()
                    .map(|&idx| case_rel_path(&prob.meta.name, prob.data[idx].id, "ans"))
                    .collect();

                match task.policy {
                    ScorePolicy::Sum => {
                        for (i, &idx) in task.items.iter().enumerate() {
                            let case = &prob.data[idx];
                            cases.push(LemonCase {
                                full_score: case.score,
                                time_limit,
                                memory_limit,
                                input_files: vec![input_files[i].clone()],
                                output_files: vec![output_files[i].clone()],
                            });
                        }
                    }
                    ScorePolicy::Min => {
                        cases.push(LemonCase {
                            full_score: task.max_score,
                            time_limit,
                            memory_limit,
                            input_files,
                            output_files,
                        });
                    }
                    ScorePolicy::Max => bail!("lemon 不支持 max 评分方法"),
                }
            }

            let chk_name = format!("chk{}", std::env::consts::EXE_SUFFIX);
            if let Some(checker) = &prob.checker {
                info!("尝试编译 SPJ");

                // 源码与依赖经 assets 读取写入 tmp，再 g++ 编译
                let src_tmp = self.tmp.path().join("chk-src.cpp");
                let mut src = assets.load(prob.idx, &checker.source)?;
                {
                    let mut f = std::fs::File::create(&src_tmp)?;
                    std::io::copy(&mut src, &mut f)?;
                }

                for dep in &checker.deps {
                    let mut dep_src = assets.load(prob.idx, dep)?;
                    let dep_name = dep.file_name().context("依赖路径缺少文件名")?.to_owned();
                    let dep_tmp = self.tmp.path().join(&dep_name);
                    {
                        let mut f = std::fs::File::create(&dep_tmp)?;
                        std::io::copy(&mut dep_src, &mut f)?;
                    }
                }

                let chk_out = self.tmp.path().join(&chk_name);
                let compile_status = Command::new("g++")
                    .arg("-o")
                    .arg(&chk_out)
                    .arg(&src_tmp)
                    .arg("-O2")
                    .arg("-std=c++23")
                    .status()?;

                if !compile_status.success() {
                    bail!("SPJ 编译错误");
                }
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("data/{}/{}", prob.meta.name, chk_name)),
                    bytes: Box::new(KeepAliveReader::new(
                        std::fs::File::open(&chk_out)?,
                        self.tmp.clone(),
                    )),
                });
            }

            let mut compilers: Map<String, Value> = Map::new();
            for (lang, _) in &doc.config.compile {
                compilers.insert(
                    compiler_for_lang(lang)?.to_string(),
                    Value::String("default".to_string()),
                );
            }

            let task_type = match prob.meta.problem_type {
                ProblemType::Program => 0,
                ProblemType::Output => 1,
                ProblemType::Interactive => bail!("lemon 不支持交互题"),
            };

            let prob_json = LemonProblem {
                answer_file_extension: "out".to_string(),
                comparison_mode: if prob.checker.is_some() { 4 } else { 1 },
                special_judge: PathBuf::from(&prob.meta.name).join(&chk_name),
                diff_arguments: "--ignore-space-change --text --brief".to_string(),
                input_file_name: format!("{}.in", prob.meta.name),
                output_file_name: format!("{}.out", prob.meta.name),
                problem_title: prob.meta.title.clone(),
                task_type,
                compiler_configuration: compilers,
                test_cases: cases,
            };

            prob_jsons.push(serde_json::to_value(&prob_json)?);
        }

        let day_cdf = json!({
            "contestTitle": doc.config.day_name,
            "contestants": Value::Array(Vec::new()),
            "tasks": prob_jsons
        });

        let cdf_str = serde_json::to_string_pretty(&day_cdf)?;
        files.push(OutputFile::File {
            path: PathBuf::from(format!("{}.cdf", doc.config.day_name)),
            bytes: Box::new(std::io::Cursor::new(cdf_str.into_bytes())),
        });

        warnings.push("受 Lemon 限制，您需要手动调整编译选项。".to_string());
        warnings.push("目前设置是默认 (default)，如需要请自行修改。".to_string());

        Ok((files, warnings))
    }
}
