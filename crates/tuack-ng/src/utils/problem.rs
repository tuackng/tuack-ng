//! 题目的派生数据。

use crate::prelude::*;
use crate::utils::target::Target;
use std::time::Duration;
use tuack_lib::problem::{ProblemMeta, ProblemType};
use tuack_utils::data::FsTestData;
use tuack_utils::runners::cpp::CppRunner;
use tuack_utils::runners::general::GeneralRunner;

/// 返回题目运行时的 [`IoMode`]：`file_io` 未配置时按文件 IO（输入输出文件名为 `<name>.in` / `<name>.out`）。
pub fn io_mode(problem: &ProblemConfig) -> IoMode {
    if problem.file_io.unwrap_or(true) {
        IoMode::File {
            input_name: format!("{}.in", problem.name),
            output_name: format!("{}.out", problem.name),
        }
    } else {
        IoMode::Stdio
    }
}

/// 按目标返回题目的数据项列表。
pub fn problem_data_items(problem: &ProblemConfig, target: Target) -> Vec<FsTestData<'_>> {
    match target {
        Target::Data => tuack_utils::data::problem_test_data(problem),
        Target::Sample => tuack_utils::data::problem_sample_data(problem),
    }
}

/// 把题目目录下的路径解析为规范绝对路径。
///
/// 绝对路径原样使用，相对路径基于题目目录。
///
/// # Errors
///
/// 路径无法解析，或目标不是普通文件时返回 `Err`。
pub fn resolve_problem_file(problem: &ProblemConfig, path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    let joined = if path.is_absolute() {
        path
    } else {
        problem.path.join(path)
    };
    let resolved = dunce::canonicalize(&joined)
        .with_context(|| format!("路径无法解析：{}", joined.display()))?;
    if !resolved.is_file() {
        bail!("目标不是普通文件：{}", resolved.display());
    }
    Ok(resolved)
}

/// 读取题目的依赖文件，返回「文件名 -> 内容」。
///
/// 文件名取自配置中的路径，保留依赖在被包含时的名字。
///
/// # Errors
///
/// 依赖文件无法解析、读取，或配置路径没有文件名时返回 `Err`。
pub fn read_deps(problem: &ProblemConfig, deps: &[String]) -> Result<IndexMap<String, Vec<u8>>> {
    let mut files = IndexMap::new();
    for dep in deps {
        let path = resolve_problem_file(problem, dep)?;
        let content =
            fs::read(&path).with_context(|| format!("读取文件失败：{}", path.display()))?;
        let name = Path::new(dep)
            .file_name()
            .with_context(|| format!("依赖路径没有文件名：{}", dep))?
            .to_string_lossy()
            .to_string();
        files.insert(name, content);
    }
    Ok(files)
}

/// 按源文件扩展名构造运行器，但不设置交互配置；交互题由 [`setup_interactive`] 设置。
///
/// # Errors
///
/// 源文件没有扩展名，或运行器构造失败时返回 `Err`。
pub fn build_runner(
    source: &Path,
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
) -> Result<Box<dyn Runner>> {
    let extension = source
        .extension()
        .context("源文件没有后缀名")?
        .to_string_lossy()
        .into_owned();

    Ok(match extension.as_str() {
        "cpp" => Box::new(CppRunner::new(
            source,
            &day_config.compile,
            problem_config.name.clone(),
        )?),
        _ => Box::new(GeneralRunner::new(
            source,
            &day_config.compile,
            problem_config.name.clone(),
            &gctx().languages,
        )?),
    })
}

/// 交互题资源的用途，决定选用哪一份 grader。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractivePurpose {
    /// 正式数据
    Data,
    /// 样例
    Sample,
    /// 数据生成
    Dmk,
}

/// 为交互题设置 grader 与 header；非交互题不做任何事。
///
/// # Errors
///
/// 交互题缺少交互配置、grader/header 无法解析，或运行器不支持交互时返回 `Err`。
pub fn setup_interactive(
    problem: &ProblemConfig,
    purpose: InteractivePurpose,
    runner: &mut dyn Runner,
) -> Result<()> {
    if problem.problem_type != tuack_config::ProblemType::Interactive {
        return Ok(());
    }
    if !runner.manifest().interactive {
        bail!("该语言不支持交互");
    }

    let interactive = problem
        .interactive
        .as_ref()
        .context("交互题目缺少交互配置")?;
    let grader = match purpose {
        InteractivePurpose::Data => &interactive.grader,
        InteractivePurpose::Sample => interactive
            .sample_grader
            .as_ref()
            .unwrap_or(&interactive.grader),
        InteractivePurpose::Dmk => interactive
            .dmk_grader
            .as_ref()
            .unwrap_or(&interactive.grader),
    };

    let grader = resolve_problem_file(problem, grader)
        .with_context(|| format!("无法解析交互 grader 源文件：{grader}"))?;
    let header = resolve_problem_file(problem, &interactive.header)
        .with_context(|| format!("无法解析交互 header 文件：{}", interactive.header))?;

    runner.set_interactive(&grader, &header)
}

/// 构造题目元信息 [`ProblemMeta`]：渲染文档与导出文档共用的那部分。
pub fn meta(problem: &ProblemConfig, day_config: &ContestDayConfig) -> ProblemMeta {
    let submit_filenames = day_config
        .compile
        .keys()
        .map(|lang_key| format!("{}.{}", problem.name, lang_key))
        .collect();

    let point_equal = if problem.runtime.data.is_empty() {
        true
    } else {
        let first = problem.runtime.data[0].score;
        problem.runtime.data.iter().all(|item| item.score == first)
    };

    ProblemMeta {
        name: problem.name.clone(),
        title: problem.title.clone(),
        problem_type: match problem.problem_type {
            tuack_config::ProblemType::Program => ProblemType::Program,
            tuack_config::ProblemType::Output => ProblemType::Output,
            tuack_config::ProblemType::Interactive => ProblemType::Interactive,
        },
        time_limit: Duration::from_secs_f64(problem.time_limit),
        memory_limit: problem.memory_limit,
        testcase: problem.runtime.data.len(),
        point_equal,
        submit_filename: submit_filenames,
    }
}
