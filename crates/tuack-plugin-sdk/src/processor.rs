//! 处理器插件：trait 与注册宏。

use extism_pdk::Error;
use tuack_lib::ren::ProcessorOutput;
use tuack_ng_parser::ast::Document;

/// 处理器插件：实现它并用 [`processor!`](crate::processor!) 注册。
///
/// 每次调用都会先由 SDK 构造新实例再执行 [`Processor::process`]，实例不跨调用保留。
pub trait Processor: Send + Sync {
    /// 构造处理器实例：由 SDK 在每次调用开始时调用。
    fn new() -> Self
    where
        Self: Sized;

    /// 由 SDK 在每次调用时调用：接收待处理的文档，返回变换后的 AST 与警告。
    ///
    /// # Errors
    ///
    /// 实现返回 `Err` 即判定本次处理失败：SDK 不写出任何输出，仅回传错误文本；
    /// 具体失败条件由实现定义。
    fn process(&self, doc: Document) -> Result<ProcessorOutput, Error>;
}

/// 注册处理器（默认导出名 `process`，插件清单按此名引用）。
///
/// 输入输出由 SDK 编解码，日志初始化与错误回传也由 SDK 接手；第二个参数可指定自定义导出名。
#[macro_export]
macro_rules! processor {
    ($ty:ty) => {
        $crate::processor!($ty, process);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            $crate::__init_logger();
            let input: $crate::Json<$crate::Document> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let processor = <$ty as $crate::Processor>::new();
            let output = match <$ty as $crate::Processor>::process(&processor, input.0) {
                Ok(output) => output,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
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
