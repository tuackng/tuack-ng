//! 题目的派生数据。

use crate::prelude::*;
use std::time::Duration;
use tuack_lib::problem::{ProblemMeta, ProblemType};

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
