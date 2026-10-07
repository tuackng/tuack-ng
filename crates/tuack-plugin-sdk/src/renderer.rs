//! 渲染器插件：trait 与注册宏。

use std::path::PathBuf;

use extism_pdk::Error;
use tuack_lib::ren::RenderDocument;
use tuack_lib::utils::output::OutputFile;

/// 渲染器插件：实现它并用 [`renderer!`](crate::renderer!) 注册。
///
/// 每次调用都会先由 SDK 构造新实例再执行 [`Renderer::render`]，实例不跨调用保留。
pub trait Renderer: Send + Sync {
    /// 构造渲染器实例：由 SDK 在每次调用开始时调用。
    fn new() -> Self
    where
        Self: Sized;

    /// 由 SDK 在每次调用时调用：接收渲染文档，返回主产物相对路径与产物文件列表。
    ///
    /// # Errors
    ///
    /// 实现返回 `Err` 即判定本次渲染失败：SDK 不上报任何产物，仅回传错误文本；
    /// 具体失败条件由实现定义。
    fn render(&self, doc: RenderDocument) -> Result<(PathBuf, Vec<OutputFile>), Error>;
}

/// 注册渲染器（默认导出名 `render`，插件清单按此名引用）。
///
/// 输入输出由 SDK 编解码，日志初始化与错误回传也由 SDK 接手；第二个参数可指定自定义导出名。
#[macro_export]
macro_rules! renderer {
    ($ty:ty) => {
        $crate::renderer!($ty, render);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            $crate::__init_logger();
            let input: $crate::Json<$crate::RenderDocument> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let renderer = <$ty as $crate::Renderer>::new();
            let (main, files) = match <$ty as $crate::Renderer>::render(&renderer, input.0) {
                Ok(x) => x,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let files = match $crate::__to_specs(files) {
                Ok(x) => x,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let output = $crate::RendererOutput { main, files };
            match $crate::extism_pdk::output($crate::Json(output)) {
                Ok(()) => 0,
                Err(e) => {
                    $crate::__report_error(&e);
                    -1
                }
            }
        }
    };
}
