use std::any::Any;
use std::io::{self, Read};

use crate::prelude::*;
use crate::utils::testlib::Arg;

/// 统一的可读流抽象：`Read + Send + 'static` 的便捷 trait（可 downcast）。
pub trait Reader: Read + Send + Any {}

impl<T: Read + Send + 'static> Reader for T {}

/// 可读数据源
pub trait Data: Send {
    /// 取输入流（每次调用返回独立新流）
    ///
    /// # Errors
    ///
    /// 输入不可打开时返回 `Err`，如文件不存在或没有读取权限。
    fn input(&self) -> io::Result<Box<dyn Reader>>;

    /// 取答案流（每次调用返回独立新流）
    ///
    /// # Errors
    ///
    /// 答案不可打开时返回 `Err`，如文件不存在或没有读取权限。
    fn answer(&self) -> io::Result<Box<dyn Reader>>;
}

/// 可写数据源
pub trait DataMut: Data {
    /// 写入该项的输入数据
    ///
    /// # Errors
    ///
    /// 写入失败时返回 `Err`，如目标路径不可创建、磁盘写满或无写权限。
    fn write_input(&self, input: Box<dyn Reader>) -> Result<()>;

    /// 写入该项的输出数据
    ///
    /// # Errors
    ///
    /// 写入失败时返回 `Err`，如目标路径不可创建、磁盘写满或无写权限。
    fn write_output(&self, output: Box<dyn Reader>) -> Result<()>;
}

/// 数据生成中的数据点：可写，并携带生成参数
pub trait DmkItem: DataMut {
    /// 生成参数
    fn args(&self) -> &IndexMap<String, Arg>;
}
