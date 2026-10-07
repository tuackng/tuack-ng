//! 插件产物回传契约（跨 wasm）。

use crate::prelude::*;

/// 插件回传的产物描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputSpec {
    /// 资产引用：以 `asset_id` 标识的资源，目标相对路径 `path`
    Asset { path: PathBuf, asset_id: u64 },
    /// 文件引用：内容位于本次调用的工作区，目标相对路径 `path`
    File { path: PathBuf },
    /// 空目录：确保存在
    Dir(PathBuf),
}

/// 渲染器插件返回：主产物相对路径与产物描述列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererOutput {
    /// 主产物的相对路径
    pub main: PathBuf,
    pub files: Vec<OutputSpec>,
}

/// 导出器插件返回：导出过程中的警告文本与产物描述列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumperOutput {
    pub warnings: Vec<String>,
    pub files: Vec<OutputSpec>,
}

pub use crate::ren::ProcessorOutput;
