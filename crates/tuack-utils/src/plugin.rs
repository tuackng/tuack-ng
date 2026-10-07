//! 插件系统：[`manifest`] 定义插件包清单，[`manager::PluginManager`] 负责发现、校验并实例化组件。

pub(crate) mod extism;
pub(crate) mod factory;
pub mod manager;
pub mod manifest;
