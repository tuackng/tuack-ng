//! 渲染处理器契约：渲染过程中改写单题 AST 的阶段。

use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 处理器：对单题 [`Document`] 做变换，返回变换结果与警告。
pub trait RenProcessor: Send + Sync {
    /// 变换单题 AST，返回变换后的 AST 与处理过程中产生的警告
    ///
    /// # Errors
    ///
    /// 变换失败时返回 `Err`：AST 含该处理器不支持的构造（如引用式链接、脚注引用、
    /// 不满足规范的表格），或外部处理器插件调用失败、其输出不符合契约。
    fn process(&self, doc: &Document) -> Result<ProcessorOutput>;
}

/// 处理器返回值
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorOutput {
    /// 变换后的题目 AST
    pub ast: Document,
    /// 处理过程中产生的警告
    pub warnings: Vec<String>,
}
