//! 实现 [`tuack_lib`] 的契约，把其数据模型落到真实的工具链与文件系统上。
//!
//! - [`runners`] / [`generators`]：编译并执行用户代码
//! - [`data`]：题目数据的读写
//! - [`ren`] / [`dump`]：渲染与导出
//! - [`mod@doc`]：题面规则检查与格式化
//! - [`plugin`]：wasm 插件宿主

pub mod assets;
pub mod checkers;
pub mod command;
pub mod data;
pub mod doc;
pub mod dump;
pub mod generators;
pub mod plugin;
pub mod prelude;
pub mod process;
pub mod ren;
pub mod runners;
pub mod utils;
pub mod validators;
