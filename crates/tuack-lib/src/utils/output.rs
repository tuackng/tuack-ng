use crate::data::Reader;
use crate::prelude::*;

/// 渲染/导出产物：文件（路径 + 字节流）或空目录。
///
/// 产物（图片/PDF/数据）可能很大，文件以 [`Reader`] 流承载，不整体进内存；
/// [`OutputFile::Dir`] 变体表达"该目录必须存在但可能没有文件"（如 Arbiter 的 final/players/result）。
///
/// 同一批产物中 [`OutputFile::File`] 的 `path` 应互不相同；重复路径的行为未定义，不应依赖。
pub enum OutputFile {
    /// 文件：相对路径 + 字节流
    File {
        /// 相对路径，如 `img/a.png`、`main.typ`
        path: PathBuf,
        bytes: Box<dyn Reader>,
    },
    /// 空目录：确保存在
    Dir(PathBuf),
}
