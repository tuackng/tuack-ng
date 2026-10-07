//! Tuack-NG 后端：题目开发的核心业务逻辑。
//!
//! - [`mod@test`] / [`dmk`]：评测与数据生成会话
//! - [`ren`] / [`dump`]：渲染与导出契约
//! - [`data`] / [`utils`]：数据源，以及运行器 / 生成器 / 校验器契约
//! - [`plugin`]：跨 wasm 边界的数据交换契约

pub mod data;
pub mod dmk;
pub mod dump;
pub mod plugin;
pub mod prelude;
pub mod problem;
pub mod ren;
pub mod test;
pub mod utils;
