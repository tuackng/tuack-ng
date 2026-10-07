//! 插件跨 wasm 边界的数据交换契约。
//!
//! 只放需要序列化过关的类型：host 函数返回（command/asset/path）与插件回传产物；
//! 共享的业务契约（[`RenderDocument`](crate::ren::RenderDocument)、
//! [`DumpDocument`](crate::dump::DumpDocument)、
//! [`OutputFile`](crate::utils::output::OutputFile)）留在 `ren`/`dump`/`utils`，
//! 由 host 与插件共同使用。

/// 插件 API 版本
///
/// 兼容性判据：major 相同且 minor 不高于本版本（patch 不参与判定）；
/// 破坏性变更升 major，兼容性新增升 minor。
pub const PLUGIN_API_VERSION: &str = "2.0.0";

mod asset;
mod command;
mod output;
mod path;

pub use asset::{AssetChunk, AssetError};
pub use command::{CommandError, CommandResult};
pub use output::{DumperOutput, OutputSpec, ProcessorOutput, RendererOutput};
pub use path::PathError;
