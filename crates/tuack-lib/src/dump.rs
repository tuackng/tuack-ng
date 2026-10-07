//! Dump 后端：导出抽象，与 ren 同构。
//!
//! [`DumpDocument`] 是不可变、可序列化的 day 级输入（要跨 WASM 边界）；实现可进行产出所
//! 必需的 I/O，但资源均经 [`AssetProvider`] 单独注入，不直接接触配置。

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::prelude::*;
use crate::problem::ProblemMeta;
use crate::utils::asset::AssetProvider;
use crate::utils::output::OutputFile;

/// 评分策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScorePolicy {
    /// 子任务得分取各测试点得分之和
    Sum,
    /// 子任务得分取各测试点中的最低分
    Min,
    /// 子任务得分取各测试点中的最高分
    Max,
}

/// day 级导出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpConfig {
    pub contest_name: String,
    pub day_name: String,
    pub dayidx: usize,
    /// 编译选项（语言，选项）
    pub compile: Vec<(String, String)>,
}

/// 单个测试点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpCase {
    pub id: u32,
    pub score: u32,
    pub subtask: u32,
    /// 输入相对路径（如 `data/1.in`）
    pub input: PathBuf,
    /// 输出相对路径（如 `data/1.ans`）
    pub output: PathBuf,
}

/// 子任务：测试点集合与评分方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpSubtask {
    /// 数据点在 data 中的下标
    pub items: Vec<usize>,
    pub max_score: u32,
    pub policy: ScorePolicy,
}

/// 样例（相对路径，如 `sample/a.in`）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpSample {
    pub input: PathBuf,
    pub output: PathBuf,
}

/// 一个待导出的文件，相对题目根的逻辑路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpFile {
    pub path: PathBuf,
}

/// 校验器信息（源文件与依赖）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpChecker {
    /// 源文件逻辑路径（如 `data/chk/chk.cpp`）
    pub source: PathBuf,
    /// 依赖文件逻辑路径列表（如 `data/chk/testlib.h`），编译时需随源码一起拷贝
    pub deps: Vec<PathBuf>,
}

/// 单题导出数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpProblem {
    /// 题目编号（与 assets 登记一致）
    pub idx: u64,
    pub meta: ProblemMeta,
    pub data: Vec<DumpCase>,
    pub subtasks: BTreeMap<u32, DumpSubtask>,
    pub samples: Vec<DumpSample>,
    /// down/ 下非样例的附加文件
    pub extra_down: Vec<DumpFile>,
    /// 校验器（源文件 + 依赖）
    pub checker: Option<DumpChecker>,
}

/// 导出文档：dumper 的唯一输入（day 级）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpDocument {
    pub config: DumpConfig,
    pub problems: Vec<DumpProblem>,
}

/// 导出器：把 [`DumpDocument`] 导出为产物文件与警告文本。
pub trait Dumper: Send + Sync {
    /// 导出文档，返回产物文件列表与导出过程中的警告文本
    ///
    /// # Errors
    ///
    /// 导出失败时返回 `Err`：资源读取失败、生成产物所必需的编译或外部命令失败、
    /// 外部导出器插件调用失败，或产物写入失败。导出过程中的警告文本经返回值的第二个
    /// 元素报告。
    fn dump(
        &self,
        doc: &DumpDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(Vec<OutputFile>, Vec<String>)>;
}
