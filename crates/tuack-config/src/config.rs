//! 配置文件数据结构

use crate::config::msgs::LoadContext;
use crate::current_location::CurrentLocation;
use crate::prelude::*;
use path_clean::PathClean;

/// 配置文件名
pub const CONFIG_FILE_NAME: &str = "conf.json";

/// 当前配置版本
pub const CONFIG_VERSION: u64 = 7;
/// 支持的最低配置版本，低于此版本的配置需先迁移
pub const CONFIG_MIN_VERSION: u64 = 3;

pub mod contest;
pub mod contestday;
pub mod lang;
pub mod migrate;
pub mod msgs;
pub mod problem;

/// 标记序列化到文件
pub struct FileView;
/// 标记序列化到其他地方，并序列化运行时信息
pub struct FullView;

pub use self::contest::ContestConfig;
pub use self::contestday::ContestDayConfig;
pub use self::problem::*;

fn find_contest_config(start_path: &Path) -> Result<PathBuf> {
    let start = dunce::canonicalize(start_path)?;

    for ancestor in start.ancestors() {
        debug!("正在查找配置文件路径：{:?}", ancestor);

        let config_path = ancestor.join(CONFIG_FILE_NAME);
        if config_path.exists() && is_contest_config(&config_path)? {
            return Ok(config_path);
        }
    }

    info!("未找到 contest 配置文件");
    bail!("未找到 contest 配置文件");
}

fn is_contest_config(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let json_value: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str())
        && folder == "contest"
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// 配置加载结果
#[derive(Debug, Clone)]
pub struct Config {
    /// 顶层比赛配置（含递归加载的比赛日与题目）
    pub config: ContestConfig,
    /// 当前工作目录对应的配置层级
    pub location: CurrentLocation,
}

/// 从 `path` 向上查找最近的 `conf.json`，加载整棵配置树并返回 [`Config`]
///
/// 加载过程逐级构造 [`ContestConfig`]、[`ContestDayConfig`] 与 [`ProblemConfig`]；
/// 比赛日或题目加载失败只向 `ctx` 记录错误并继续，不影响其余层级。
/// `path` 不存在或向上找不到配置文件时返回 `Ok(None)`，不视为错误。
///
/// # Errors
///
/// - 顶层配置无法读取或解析、缺少 `version` 字段，或版本不在
///   [`CONFIG_MIN_VERSION`] 与 [`CONFIG_VERSION`] 之间（详见 [`ContestConfig::load`]）；
/// - 顶层配置的迁移失败，包括不存在对应迁移器、迁移需人工确认
///   （[`MigraterMetadata::force`] 为 `true`）而 `ctx` 未启用强制迁移；
/// - 顶层配置路径没有父目录。
///
/// [`MigraterMetadata::force`]: crate::config::migrate::base::MigraterMetadata::force
pub fn load_config(ctx: &mut LoadContext, path: &Path) -> Result<Option<Config>> {
    let config_path = match find_contest_config(path) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    let canonicalize_path = dunce::canonicalize(path)?.to_path_buf();

    ctx.enter();

    let mut config = ContestConfig::load(ctx, &config_path)?;

    ctx.set_name(format!("[contest] {}", config.name));

    let mut location: CurrentLocation = CurrentLocation::None;

    if canonicalize_path.starts_with(config_path.parent().unwrap()) {
        location = CurrentLocation::Root;
    }

    let parent_dir = config_path.parent().context("无法获取配置文件父目录")?;

    // 递归加载子配置
    for day_name in config.subdir.clone() {
        let day_path = parent_dir.join(&day_name).join(CONFIG_FILE_NAME).clean();

        ctx.enter();
        let mut dayconfig = match ContestDayConfig::load(ctx, &day_path) {
            Ok(config) => config,
            Err(e) => {
                ctx.ret();
                ctx.emit_error(format!("比赛日 {} 加载失败：{}", day_name.cyan(), e));
                continue;
            }
        };

        ctx.set_name(format!("[day] {}", dayconfig.name));

        if canonicalize_path.starts_with(day_path.parent().unwrap()) {
            location = CurrentLocation::Day(day_name.to_string());
        }

        // 递归加载题目配置
        let day_parent_dir = day_path.parent().context("无法获取配置文件父目录")?;
        for problem_name in dayconfig.subdir.clone() {
            let problem_path = day_parent_dir
                .join(&problem_name)
                .join(CONFIG_FILE_NAME)
                .clean();

            ctx.enter();
            let mut problemconfig = match ProblemConfig::load(ctx, &problem_path) {
                Ok(config) => config,
                Err(e) => {
                    ctx.ret();
                    ctx.emit_error(format!("题目 {} 加载失败：{}", problem_name.cyan(), e));
                    continue;
                }
            };

            ctx.set_name(format!("[problem] {}", problemconfig.name));
            problemconfig.use_pretest = dayconfig.use_pretest.or(config.use_pretest);
            problemconfig.noi_style = dayconfig.noi_style.or(config.noi_style);
            problemconfig.file_io = if problemconfig.problem_type == ProblemType::Interactive {
                // 交互强制使用 Stdio
                Some(false)
            } else {
                None
            }
            .or(dayconfig.file_io)
            .or(config.file_io);

            if canonicalize_path.starts_with(problem_path.parent().unwrap()) {
                location = CurrentLocation::Problem(day_name.to_string(), problem_name.to_string());
            }

            dayconfig
                .subconfig
                .inner_mut()
                .insert(problem_name.to_string(), problemconfig);

            ctx.ret(); // problem
        }

        config
            .subconfig
            .inner_mut()
            .insert(day_name.to_string(), dayconfig);

        ctx.ret(); // day
    }

    ctx.ret(); // contest

    Ok(Some(Config { config, location }))
}

/// 将 [`ContestConfig`] 及其下属比赛日、题目配置序列化后写回各自的 `conf.json`
///
/// 比赛根目录由 `base_path` 指定，比赛日与题目配置分别写入
/// `base_path/<比赛日>/conf.json` 与 `base_path/<比赛日>/<题目>/conf.json`。
///
/// # Errors
///
/// - `base_path` 不存在；
/// - 某个比赛日或题目目录不存在；
/// - 配置序列化失败或写入文件失败。
pub fn save_config(config: &ContestConfig, base_path: &Path) -> Result<()> {
    if !base_path.exists() {
        bail!("基础目录 {} 不存在", base_path.display());
    }

    // 保存主配置文件
    let main_config_path = base_path.join(CONFIG_FILE_NAME);
    let main_config_json = config.save()?;
    fs::write(&main_config_path, main_config_json)?;

    // 保存每个比赛日的配置
    for (day_name, day_config) in config.subconfig.iter() {
        let day_path = base_path.join(day_name);

        if !day_path.exists() {
            bail!("比赛日目录 {} 不存在", day_path.display());
        }

        let day_config_path = day_path.join(CONFIG_FILE_NAME);
        let day_config_json = day_config.save()?;
        fs::write(&day_config_path, day_config_json)?;

        // 保存每个题目的配置
        for (problem_name, problem_config) in day_config.subconfig.iter() {
            let problem_path = day_path.join(problem_name);

            if !problem_path.exists() {
                bail!("题目目录 {} 不存在", problem_path.display());
            }

            let problem_config_path = problem_path.join(CONFIG_FILE_NAME);
            let problem_config_json = problem_config.save()?;
            fs::write(&problem_config_path, problem_config_json)?;
        }
    }

    Ok(())
}
