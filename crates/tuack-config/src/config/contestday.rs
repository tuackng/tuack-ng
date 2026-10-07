use crate::config::migrate::base::MIGRATERS;
use crate::config::msgs::LoadContext;
use crate::config::problem::ProblemConfig;
use crate::config::{CONFIG_MIN_VERSION, CONFIG_VERSION};
use crate::prelude::*;
use indexmap::IndexMap;

/// 比赛日层级配置
#[derive(Debug, Clone, DeserializeMany, SerializeMany)]
#[serde_many(file = "FileView", full = "FullView")]
#[serde(file(rename_all = "kebab-case"), full(rename_all = "kebab-case"))]
pub struct ContestDayConfig {
    /// 配置文件版本
    pub version: u32,
    /// 层级标识，此处恒为 `day`
    pub folder: String,
    /// 比赛日名称
    pub name: String,
    /// 题目目录名列表
    pub subdir: Vec<String>,
    /// 比赛日标题
    pub title: String,
    /// 各语言的编译选项，键为语言 key（如 `cpp`）
    pub compile: IndexMap<String, String>,
    /// 比赛开始时间，格式 `[年，月，日，时，分，秒]`
    #[serde(file(rename = "start time"), full(rename = "start time"))]
    pub start_time: Option<[u32; 6]>,
    /// 比赛结束时间，格式 `[年，月，日，时，分，秒]`
    #[serde(file(rename = "end time"), full(rename = "end time"))]
    pub end_time: Option<[u32; 6]>,
    /// 是否启用 pretest，题目未配置时作为其默认值
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub use_pretest: Option<bool>,
    /// 是否 NOI 风格，题目未配置时作为其默认值
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub noi_style: Option<bool>,
    /// 是否使用文件 IO，题目未配置时作为其默认值
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub file_io: Option<bool>,

    // 运行时信息
    /// 各题目的子配置（运行时填充）
    #[serde(file(skip))]
    pub subconfig: IndexMapMany<String, ProblemConfig>,
    /// 配置文件所在目录（运行时填充）
    #[serde(file(skip))]
    pub path: PathBuf,
}

impl ContestDayConfig {
    /// 加载比赛日层级配置，必要时按迁移链逐级升级到 [`CONFIG_VERSION`]
    ///
    /// 版本低于 [`CONFIG_VERSION`] 时依次尝试 [`MIGRATERS`] 中登记的迁移器，并在 `ctx`
    /// 中记录本次加载发生过迁移与各迁移器的通知信息。
    ///
    /// # Errors
    ///
    /// - 配置文件无法读取或解析为 JSON；
    /// - 缺少 `version` 或 `folder` 字段，或 `folder` 不为 `day`；
    /// - 版本低于 [`CONFIG_MIN_VERSION`]（Tuack 旧配置）或高于 [`CONFIG_VERSION`]；
    /// - 不存在从当前版本出发的迁移器；
    /// - 迁移需人工确认（[`MigraterMetadata::force`] 为 `true`）而 `ctx` 未启用强制迁移；
    /// - 迁移过程中配置解析失败，或配置文件没有父目录。
    ///
    /// [`MigraterMetadata::force`]: crate::config::migrate::base::MigraterMetadata::force
    pub fn load(ctx: &mut LoadContext, config_path: &Path) -> Result<Self> {
        // 读取并验证每日配置文件
        let content = fs::read_to_string(config_path)?;
        let mut json: serde_json::Value = serde_json::from_str(&content)?;

        let mut version = json
            .get("version")
            .and_then(|v| v.as_u64())
            .context("配置文件缺少版本号")?;

        // 检查版本
        if version < CONFIG_MIN_VERSION {
            bail!(
                "配置文件版本过低，可能是 Tuack 的配置文件。请迁移到 Tuack-NG 配置文件格式再使用。"
            );
        }

        if version > CONFIG_VERSION {
            bail!("配置文件版本过高，可能是新版本的配置文件。请检查是否有新版本。");
        }

        let folder = json
            .get("folder")
            .and_then(|v| v.as_str())
            .context("配置文件缺少 `folder` 字段")?;

        if folder != "day" {
            bail!("配置文件层级错误。预期 `day`，读到 `{}`", folder);
        }

        while version < CONFIG_VERSION {
            match MIGRATERS.get(&(version as i32)) {
                Some(migrater) => {
                    if migrater.metadata().force && !ctx.force_migrate() {
                        bail!(
                            "配置文件已经过时且无法自动迁移。你需要使用 `tuack-ng conf migrate` 手动迁移。"
                        )
                    } else {
                        let from_ver = version as i32;
                        json = migrater.migrate_day(json, config_path.parent().unwrap())?;
                        version = json
                            .get("version")
                            .and_then(|v| v.as_u64())
                            .context("配置文件缺少版本号")?;
                        ctx.migrated = true;
                        if let Some(notice) = migrater.metadata().notice {
                            ctx.migrated_notices.entry(from_ver).or_insert(notice);
                        }
                    }
                }
                None => bail!("不存在配置文件版本 {} 的迁移", version),
            }
        }

        // 反序列化主配置
        let mut config: ContestDayConfig =
            serde_json::from_value::<AsSerde<ContestDayConfig, FileView>>(json)?.into_inner();

        config.path = config_path
            .parent()
            .context("无法获取配置文件父目录")?
            .into();

        Ok(config)
    }

    /// 将本层级配置序列化为 `conf.json` 的 JSON 文本，仅含文件视图字段
    ///
    /// 序列化失败时返回 `Err`。
    pub fn save(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&AsSerde::<
            ContestDayConfig,
            FileView,
        >::new(self.clone()))?)
    }
}
