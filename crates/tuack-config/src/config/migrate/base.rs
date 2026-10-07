use crate::prelude::*;

/// 迁移器的元信息
pub struct MigraterMetadata {
    /// 是否需要用户显式确认
    ///
    /// 为 `true` 表示迁移涉及 `conf.json` 之外的改动：配置加载默认拒绝自动执行，
    /// 需由 [`LoadContext::new_force_migrate`](crate::config::msgs::LoadContext::new_force_migrate)
    /// 创建的上下文显式授权。
    pub force: bool,
    /// 迁移时向用户展示的通知信息
    pub notice: Option<&'static str>,
}

/// 数据迁移器：把一个源版本的配置升级到下一个版本
pub trait Migrater: Send + Sync {
    /// 迁移器的元信息
    fn metadata(&self) -> MigraterMetadata;
    /// 迁移比赛层级配置
    ///
    /// # Errors
    ///
    /// 具体失败条件由实现定义。
    fn migrate_contest(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
    /// 迁移比赛日层级配置
    ///
    /// # Errors
    ///
    /// 具体失败条件由实现定义。
    fn migrate_day(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
    /// 迁移题目层级配置
    ///
    /// # Errors
    ///
    /// 具体失败条件由实现定义。
    fn migrate_problem(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
}

use indexmap::IndexMap;
use std::sync::LazyLock;

/// 迁移器注册表，键为源版本号，值为把该版本升级到下一版本的迁移器
pub static MIGRATERS: LazyLock<IndexMap<i32, Box<dyn Migrater>>> = LazyLock::new(|| {
    let mut map = IndexMap::<i32, Box<dyn Migrater>>::new();
    map.insert(3, Box::new(crate::config::migrate::v3::V3Migrater));
    map.insert(4, Box::new(crate::config::migrate::v4::V4Migrater));
    map.insert(5, Box::new(crate::config::migrate::v5::V5Migrater));
    map.insert(6, Box::new(crate::config::migrate::v6::V6Migrater));
    map
});
