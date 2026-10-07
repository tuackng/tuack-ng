//! 产物转换：把插件产物列表转成可回传的 `OutputSpec`。

use extism_pdk::Error;
use tuack_lib::plugin::OutputSpec;
use tuack_lib::utils::output::OutputFile;

/// 把产物文件列表转成可回传的 [`OutputSpec`]：资产保留句柄（host-to-host），
/// 其余内容写入 WASI 工作区 `/out/<path>`（供 [`renderer!`](crate::renderer!) /
/// [`dumper!`](crate::dumper!) 宏内部使用）。
///
/// # Errors
///
/// 写入文件产物失败时返回 [`Error`]：无法创建 `/out` 下的目录、无法创建目标文件或拷贝
/// 字节失败（`std::fs` / `std::io` 错误）；资产与空目录产物不经文件系统，不会失败。
///
/// 文件产物要求插件清单声明 `wasi = true`，否则插件内没有 WASI 文件访问。
#[doc(hidden)]
pub fn __to_specs(files: Vec<OutputFile>) -> Result<Vec<OutputSpec>, Error> {
    let mut specs = Vec::new();
    for file in files {
        match file {
            OutputFile::File { path, mut bytes } => {
                let is_asset = {
                    let any: &dyn std::any::Any = &*bytes;
                    any.is::<crate::AssetReader>()
                };
                if is_asset {
                    let any: Box<dyn std::any::Any> = bytes;
                    let asset = any
                        .downcast::<crate::AssetReader>()
                        .expect("已确认是 AssetReader");
                    specs.push(OutputSpec::Asset {
                        path,
                        asset_id: asset.into_id(),
                    });
                } else {
                    let dest = std::path::Path::new("/out").join(&path);
                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut f = std::fs::File::create(&dest)?;
                    std::io::copy(&mut bytes, &mut f)?;
                    specs.push(OutputSpec::File { path });
                }
            }
            OutputFile::Dir(path) => specs.push(OutputSpec::Dir(path)),
        }
    }
    Ok(specs)
}
