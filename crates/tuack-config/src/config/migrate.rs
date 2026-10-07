//! 配置版本迁移器，按 [`base::MIGRATERS`] 登记的链路把旧版本配置逐级升级到当前版本。

pub mod base;
pub mod v3;
pub mod v4;
pub mod v5;
pub mod v6;
