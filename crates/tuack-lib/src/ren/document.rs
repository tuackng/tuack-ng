use tuack_ng_parser::ast::Document;

use crate::prelude::*;
use crate::problem::ProblemMeta;

/// 支持的语言
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportLanguage {
    pub name: String,
    pub compile_options: String,
}

/// 比赛日起止时间
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateInfo {
    /// 起始时间 `[年，月，日，时，分，秒]`
    pub start: [u32; 6],
    /// 结束时间 `[年，月，日，时，分，秒]`
    pub end: [u32; 6],
}

/// 渲染参数：模板展开与渲染共用的选项。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenParams {
    pub use_pretest: bool,
    pub noi_style: bool,
    pub file_io: bool,
}

/// 渲染配置：比赛级元信息与渲染参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenConfig {
    pub title: String,
    pub short_title: String,
    pub day_key: String,
    pub dayname: String,
    pub date: Option<DateInfo>,
    #[serde(flatten)]
    pub params: RenParams,
    pub support_languages: Vec<SupportLanguage>,
}

/// 一道题的完整渲染数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    /// 题目编号（0 起）
    pub idx: u64,
    pub meta: ProblemMeta,
    pub ast: Document,
    /// 图片重写映射：文档原始 URL -> 输出相对路径（如 `img/{题号}/...`）。
    pub images: IndexMap<PathBuf, PathBuf>,
}

/// 渲染文档：[`Renderer`](crate::ren::Renderer) 的输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDocument {
    pub config: RenConfig,
    pub problems: Vec<Problem>,
    pub precaution: Option<Document>,
}
