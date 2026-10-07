//! 插件日志门面：把 `log` crate 的记录转发到宿主（按级别）。

use crate::host::plugin_log;

struct PluginLogger;

static LOGGER: PluginLogger = PluginLogger;

impl log::Log for PluginLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        // 级别数值是插件与宿主 `plugin_log` 的约定，修改须两端同步
        let level = match record.level() {
            log::Level::Error => 4,
            log::Level::Warn => 3,
            log::Level::Info => 2,
            log::Level::Debug => 1,
            log::Level::Trace => 0,
        };
        // SAFETY: plugin_log 由宿主注册，签名与 host.rs 中 extern 块的声明一致
        let _ = unsafe { plugin_log(level, record.args().to_string()) };
    }

    fn flush(&self) {}
}

/// 初始化插件日志：注册日志门面并把记录转发到宿主；重复调用只生效一次。
#[doc(hidden)]
pub fn __init_logger() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Trace);
    });
}
