use crate::prelude::*;
use tuack_ng_parser::ast::Document;

pub mod autocorrect;
pub mod html;
pub mod invisible;
pub mod latex;
pub mod samples_not_found;
pub mod samples_should_be_external;
pub mod samples_too_large;

// 格式化

pub struct FormatManifest {
    pub name: String,
    pub description: String,
    /// 是否支持 Markdown 文本输入
    pub markdown_formatter: bool,
    /// 是否支持 AST 输入
    pub ast_formatter: bool,
}

/// 规则产出的文件（`path` 相对题目根，如 `sample/1.in`）
///
/// 规则本身不写盘，文件内容随返回值一并给出。
pub struct RuleFile {
    pub path: PathBuf,
    pub content: Vec<u8>,
}

/// 文档格式化规则
///
/// 每个规则只支持一种输入：[`FormatManifest`] 中标示为不支持的方向需实现为 `unreachable!()`。
pub trait FormatRule {
    /// 处理 Markdown 文本，返回处理后的文本、可能被改写的题目配置与规则产出的文件
    ///
    /// # Panics
    ///
    /// `manifest` 中 `markdown_formatter` 为 `false` 时不应调用本方法。
    ///
    /// # Errors
    ///
    /// 规则自身处理失败（如底层格式化器报错）时返回 `Err`；产出的文件以 [`RuleFile`] 随返回值
    /// 给出。
    fn apply_markdown(
        &self,
        doc: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig, Vec<RuleFile>)>;
    /// 处理解析后的文档，返回值含义同 [`FormatRule::apply_markdown`]
    ///
    /// # Panics
    ///
    /// `manifest` 中 `ast_formatter` 为 `false` 时不应调用本方法。
    ///
    /// # Errors
    ///
    /// 规则自身处理失败（如底层解析或变换报错）时返回 `Err`；产出的文件同样以 [`RuleFile`] 返回。
    fn apply_ast(
        &self,
        doc: Document,
        problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig, Vec<RuleFile>)>;
    /// 规则元信息
    fn manifest(&self) -> FormatManifest;
}

// 检查
#[derive(Clone, Copy, PartialEq)]
pub enum CheckImportance {
    Warn,
    Error,
}

/// 一条检查诊断
pub struct CheckInfo {
    /// 主定位（源码字节区间），None 表示无精确位置
    pub span: Option<tuack_ng_parser::span::Span>,
    /// 次要定位，指向与主定位相关的另一处
    pub secondary_span: Option<tuack_ng_parser::span::Span>,
    /// 诊断正文
    pub info: String,
    /// 补充说明，可多行
    pub note: Option<String>,
    pub importance: CheckImportance,
}

impl CheckInfo {
    pub fn new(
        span: Option<tuack_ng_parser::span::Span>,
        info: String,
        importance: CheckImportance,
    ) -> Self {
        Self {
            span,
            secondary_span: None,
            info,
            note: None,
            importance,
        }
    }
}

/// 检查结果
pub enum CheckResult {
    /// 只报告问题数量，不含具体位置
    #[allow(unused)]
    Untagged(usize),
    /// 逐条报告诊断
    Tagged(Vec<CheckInfo>),
}

pub struct CheckManifest {
    pub name: String,
    pub description: String,
    /// 是否支持 Markdown 文本输入
    pub markdown_checker: bool,
    /// 是否支持 AST 输入
    pub ast_checker: bool,
}

/// 文档检查规则
///
/// 每个规则只支持一种输入：[`CheckManifest`] 中标示为不支持的方向需实现为 `unreachable!()`。
pub trait CheckRule {
    /// 检查 Markdown 文本
    ///
    /// # Panics
    ///
    /// `manifest` 中 `markdown_checker` 为 `false` 时不应调用本方法。
    ///
    /// # Errors
    ///
    /// 检查无法完成（如底层检查器报错）时返回 `Err`；诊断以 [`CheckResult`] 返回，规则不修改文本。
    fn check_markdown(&self, doc: &str, problem_config: &ProblemConfig) -> Result<CheckResult>;
    /// 检查解析后的文档；`source` 为对应 Markdown 源码，用于把 AST 的字节偏移换算回整行区间
    ///
    /// # Panics
    ///
    /// `manifest` 中 `ast_checker` 为 `false` 时不应调用本方法。
    ///
    /// # Errors
    ///
    /// 检查无法完成时返回 `Err`；诊断以 [`CheckResult`] 返回，规则不修改文档。
    fn check_ast(
        &self,
        doc: &Document,
        source: &str,
        problem_config: &ProblemConfig,
    ) -> Result<CheckResult>;
    /// 规则元信息
    fn manifest(&self) -> CheckManifest;
}
