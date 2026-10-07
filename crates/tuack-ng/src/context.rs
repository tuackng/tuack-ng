use crate::prelude::*;
use indicatif::MultiProgress;
use std::sync::OnceLock;
use tuack_config::Config;
use tuack_config::lang::Language;
use tuack_config::msgs::LoadContext;
use tuack_utils::plugin::manager::PluginManager;

/// 进程级全局运行时状态。
pub struct Context {
    /// 资源目录，按搜索优先级从高到低排列。
    pub assets_dirs: Vec<PathBuf>,
    /// 共享进度条组；输出消息时会将其挂起，避免两类输出互相覆盖。
    pub multiprogress: MultiProgress,

    /// 当前工程的配置加载结果；向上找不到 `conf.json` 时为 `None`。
    pub config: Option<Config>,
    /// 配置加载过程中累积的消息与迁移标记。
    pub loadctx: LoadContext,
    /// 语言表，取自资源目录中首个 `langs.json`。
    pub languages: IndexMap<String, Language>,
    /// 已发现的插件及其加载状态
    pub plugins: PluginManager,
}

pub static GLOBAL_CONTEXT: OnceLock<Context> = OnceLock::new();

/// 初始化全局上下文。
///
/// # Errors
///
/// [`GLOBAL_CONTEXT`] 已被写入（重复初始化）时返回错误。
pub fn setup_context(x: Context) -> Result<()> {
    if GLOBAL_CONTEXT.set(x).is_err() {
        bail!("Already initialized");
    }
    Ok(())
}

/// 取得全局上下文的引用。
///
/// # Panics
///
/// 全局上下文尚未初始化（未调用 [`setup_context`]）时 panic。
pub fn gctx() -> &'static Context {
    GLOBAL_CONTEXT.get().expect("Not initialized")
}
