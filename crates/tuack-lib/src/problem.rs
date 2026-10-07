//! 题目层面的共用契约：渲染与导出共用的 [`ProblemType`] 与 [`ProblemMeta`]。

use bytesize::ByteSize;
use std::time::Duration;

use crate::prelude::*;

/// 题目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemType {
    /// 传统型：提交程序，读写测试数据
    Program,
    /// 提交答案型：只评测提交的输出文件
    Output,
    /// 交互型：通过与 grader 交互作答
    Interactive,
}

/// 题目元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemMeta {
    /// 题目 (英文) 名称
    pub name: String,
    pub title: String,
    pub problem_type: ProblemType,
    pub time_limit: Duration,
    pub memory_limit: ByteSize,
    /// 测试点数量
    pub testcase: usize,
    /// 各测试点分数是否相等
    pub point_equal: bool,
    /// 可提交的文件名（如 ["a.cpp", "a.py"]）
    pub submit_filename: Vec<String>,
}
