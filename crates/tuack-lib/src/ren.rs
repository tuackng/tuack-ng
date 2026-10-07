//! 渲染抽象，与 dump 同构。
//!
//! [`RenderDocument`] 是不可变输入，[`Renderer::render`] 产出 [`OutputFile`] 列表。实现可
//! 进行渲染所必需的 I/O（写临时目录、调用外部命令），但资源均经 [`AssetProvider`] 取得，
//! 且不写最终输出目录。

pub mod document;
pub mod processor;

pub use crate::problem::{ProblemMeta, ProblemType};
use crate::utils::output::OutputFile;
pub use document::{DateInfo, Problem, RenConfig, RenParams, RenderDocument, SupportLanguage};
pub use processor::{ProcessorOutput, RenProcessor};

use crate::prelude::*;
use crate::utils::asset::AssetProvider;

/// 渲染器：把 [`RenderDocument`] 渲染为产物文件，并给出主产物相对路径。
pub trait Renderer: Send + Sync {
    /// 渲染文档，返回主产物相对路径与全部产物文件
    ///
    /// # Errors
    ///
    /// 渲染失败时返回 `Err`：资源读取失败、模板展开或排版失败（如依赖的外部命令缺失或
    /// 以非零码退出）、外部渲染器插件调用失败，或产物写入失败。
    fn render(
        &self,
        doc: &RenderDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(PathBuf, Vec<OutputFile>)>;
}
