//! 配置文件（`conf.json`）的数据结构，以及其加载、保存与版本迁移。
//!
//! - [`Config`] / [`load_config`]：加载入口，并向上定位当前层级
//! - [`config`]：contest / day / problem 三层配置结构
//! - [`current_location`]：当前工作目录对应的配置层级

pub mod config;
pub mod current_location;
pub mod prelude;

pub use config::problem::*;
pub use config::{
    CONFIG_FILE_NAME, CONFIG_MIN_VERSION, CONFIG_VERSION, Config, FileView, FullView, load_config,
    save_config,
};
pub use config::{ContestConfig, ContestDayConfig};
pub use config::{contest, contestday, lang, migrate, msgs, problem};
pub use current_location::CurrentLocation;
