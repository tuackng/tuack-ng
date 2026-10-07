//! 导出器插件：trait 与注册宏。

use extism_pdk::Error;
use tuack_lib::dump::DumpDocument;
use tuack_lib::utils::output::OutputFile;

/// 导出器插件：实现它并用 [`dumper!`](crate::dumper!) 注册。
///
/// [`Dumper::dump`] 接收导出文档，返回产物文件列表与导出警告；每次调用都会先由 SDK 构造
/// 新实例，实例不跨调用保留。
pub trait Dumper: Send + Sync {
    /// 构造导出器实例：由 SDK 在每次调用开始时调用。
    fn new() -> Self
    where
        Self: Sized;

    /// 由 SDK 在每次调用时调用。
    ///
    /// # Errors
    ///
    /// 实现返回 `Err` 即判定本次导出失败：SDK 不上报任何产物或警告，仅回传错误文本；
    /// 具体失败条件由实现定义。
    fn dump(&self, doc: DumpDocument) -> Result<(Vec<OutputFile>, Vec<String>), Error>;
}

/// 注册导出器（默认导出名 `dump`，插件清单按此名引用）。
///
/// 输入输出由 SDK 编解码，日志初始化与错误回传也由 SDK 接手；第二个参数可指定自定义导出名。
#[macro_export]
macro_rules! dumper {
    ($ty:ty) => {
        $crate::dumper!($ty, dump);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            $crate::__init_logger();
            let input: $crate::Json<$crate::DumpDocument> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let dumper = <$ty as $crate::Dumper>::new();
            let (files, warnings) = match <$ty as $crate::Dumper>::dump(&dumper, input.0) {
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
            let output = $crate::DumperOutput { warnings, files };
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
