use crate::data::Reader;
use crate::prelude::*;

/// 资源提供方：按"题目编号 + 逻辑路径"惰性返回资源字节流。
///
/// 具体的资源来源（如文件系统）不暴露在接口上。
pub trait AssetProvider: Send + Sync {
    /// 获取第 `idx` 题的资源 `path`
    ///
    /// # Errors
    ///
    /// 以下情况返回 `Err`：该题号未登记、`path` 为绝对路径或含 `..` 向上分量、
    /// 以及 `path` 指向的资源打不开（不存在或无读取权限）。
    fn load(&self, idx: u64, path: &Path) -> Result<Box<dyn Reader>>;
}
