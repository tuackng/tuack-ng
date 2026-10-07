use crate::{
    config::{CONFIG_MIN_VERSION, CONFIG_VERSION, migrate::base::MIGRATERS, msgs::LoadContext},
    prelude::*,
};
use bytesize::ByteSize;
use indexmap::IndexMap;
use tuack_lib::utils::testlib::Arg;

/// 运行时内容（展开结果，与静态内容在类型层面完全区分）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProblemRuntime {
    /// 样例（展开后）
    pub samples: Vec<ExpandedSampleItem>,
    /// 数据（继承全局参数后，尚未展开）
    pub inherited_data: Vec<DataItem>,
    /// 数据（展开后）
    pub data: Vec<ExpandedDataItem>,
    /// Subtask 配置（展开后）
    pub subtasks: BTreeMap<u32, SubtaskItem>,
}

/// 题目层级配置：配置文件中的静态字段与加载时展开的运行时内容
#[derive(Debug, Clone, DeserializeMany, SerializeMany)]
#[serde_many(file = "FileView", full = "FullView")]
#[serde(file(rename_all = "kebab-case"), full(rename_all = "kebab-case"))]
pub struct ProblemConfig {
    /// 配置文件版本：低于 [`CONFIG_VERSION`] 时需迁移，低于 [`CONFIG_MIN_VERSION`] 会被拒绝
    pub version: u32,
    /// 文件夹类型，在此处应为 `problem`
    pub folder: String,
    /// 题目类型
    #[serde(file(rename = "type"), full(rename = "type"))]
    pub problem_type: ProblemType,
    /// 题目 (英文) 名称
    pub name: String,
    /// 题目标题
    pub title: String,
    /// 时间限制（秒）
    #[serde(file(rename = "time limit"), full(rename = "time limit"))]
    pub time_limit: f64,
    /// 空间限制（支持 `256 MiB` 等人类可读单位）
    #[serde(file(rename = "memory limit"), full(rename = "memory limit"))]
    pub memory_limit: ByteSize,
    /// 数据生成行为
    pub dmk: DmkConfig,
    /// 数据点参数（全局部分），各数据点会在此基础上覆盖
    #[serde(file(default, skip_serializing_if = "IndexMap::is_empty"))]
    pub args: IndexMap<String, Arg>,
    /// 交互配置，[`ProblemType::Interactive`] 题目必需
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub interactive: Option<InteractiveConfig>,
    /// 生成器配置
    #[serde(file(default))]
    pub generator: Option<GeneratorConfigPair>,

    /// 样例（原始）
    #[serde(full(rename = "orig_samples"))]
    pub samples: Vec<SampleItem>,
    /// 数据 (原始)
    #[serde(full(rename = "orig_data"))]
    pub data: Vec<DataItem>,
    /// Subtask 配置 (原始)，键为 Subtask ID
    #[serde(file(default), full(rename = "orig_subtasks"))]
    pub subtasks: BTreeMap<u32, ScorePolicy>,
    /// 测试用例，键为用例名称
    #[serde(file(default, skip_serializing_if = "IndexMap::is_empty"))]
    pub tests: IndexMap<String, TestCase>,
    /// Checker 配置
    #[serde(file(default))]
    pub checker: Option<CheckerConfigPair>,
    /// Validator 配置
    #[serde(file(default))]
    pub validator: Option<ValidatorConfigPair>,

    /// 是否有 pretest，目前没有用途
    #[serde(file(skip))]
    pub use_pretest: Option<bool>,
    /// 是否是 NOI 风格
    #[serde(file(skip))]
    pub noi_style: Option<bool>,
    /// 是否使用文件 IO
    #[serde(file(skip))]
    pub file_io: Option<bool>,
    /// 当前配置所在路径，运行时生成
    #[serde(file(skip))]
    pub path: PathBuf,

    /// 运行时内容（展开结果，独立类型）
    #[serde(file(skip), full(flatten))]
    pub runtime: ProblemRuntime,
}

impl ProblemConfig {
    /// 加载题目层级配置，迁移到当前版本后展开样例、数据与 Subtask 到 [`ProblemRuntime`]
    ///
    /// 版本低于 [`CONFIG_VERSION`] 时依次尝试 [`MIGRATERS`] 中登记的迁移器，并在 `ctx`
    /// 中记录本次加载发生过迁移与各迁移器的通知信息。数据点引用不存在的 Subtask ID 时
    /// 只向 `ctx` 记录警告，不影响加载结果。
    ///
    /// # Errors
    ///
    /// - 配置文件无法读取或解析为 JSON；
    /// - 缺少 `version` 或 `folder` 字段，或 `folder` 不为 `problem`；
    /// - 版本低于 [`CONFIG_MIN_VERSION`]（Tuack 旧配置）或高于 [`CONFIG_VERSION`]；
    /// - 不存在从当前版本出发的迁移器；
    /// - 迁移需人工确认（[`MigraterMetadata::force`] 为 `true`）而 `ctx` 未启用强制迁移；
    /// - 迁移过程中旧格式的 `samples`、`data` 字段无法解析；
    /// - [`ProblemType::Interactive`] 题目缺少 `interactive` 配置；
    /// - 配置文件没有父目录。
    ///
    /// [`MigraterMetadata::force`]: crate::config::migrate::base::MigraterMetadata::force
    pub fn load(ctx: &mut LoadContext, config_path: &Path) -> Result<Self> {
        // 读取并验证问题配置文件
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

        if folder != "problem" {
            bail!("配置文件层级错误。预期 `problem`，读到 `{}`", folder);
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
                        json = migrater.migrate_problem(json, config_path.parent().unwrap())?;
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
        let mut config: ProblemConfig =
            serde_json::from_value::<AsSerde<ProblemConfig, FileView>>(json)?.into_inner();

        // 展开样例
        let mut expand_samples: Vec<ExpandedSampleItem> = vec![];
        for sample in &config.samples {
            let mut merged = config.args.clone();
            merged.extend(sample.args.clone());
            expand_samples.push(ExpandedSampleItem {
                id: sample.id,
                input: sample.input_path(),
                output: sample.output_path(),
                args: merged,
                dmk: sample.dmk.unwrap_or(config.dmk),
            });
        }

        // 继承全局参数（尚未展开）
        let mut inherited_data: Vec<DataItem> = vec![];
        for data in &config.data {
            let mut merged = config.args.clone();
            let mut item = data.clone();
            match &mut item {
                DataItem::Single(item) => {
                    merged.extend(item.orig_args.clone());
                    item.orig_args = merged;
                }
                DataItem::Bundle(item) => {
                    merged.extend(item.orig_args.clone());
                    item.orig_args = merged;
                }
            }
            inherited_data.push(item);
        }

        // 展开数据点（基于继承后的数据）
        let mut expand_data: Vec<ExpandedDataItem> = vec![];
        for data in &inherited_data {
            match data {
                DataItem::Single(item) => {
                    expand_data.push(ExpandedDataItem {
                        id: item.id,
                        score: item.score,
                        subtask: item.subtask,
                        input: item
                            .input
                            .clone()
                            .unwrap_or_else(|| format!("{}.in", item.id)),
                        output: item
                            .output
                            .clone()
                            .unwrap_or_else(|| format!("{}.ans", item.id)),
                        orig_args: item.orig_args.clone(),
                        args: item.orig_args.clone(),
                        dmk: item.dmk.unwrap_or(config.dmk),
                    });
                }
                DataItem::Bundle(item) => {
                    for id in &item.id {
                        expand_data.push(ExpandedDataItem {
                            id: *id as u32,
                            score: item.score,
                            subtask: item.subtask,
                            input: format!("{}.in", id),
                            output: format!("{}.ans", id),
                            orig_args: item.orig_args.clone(),
                            args: item.orig_args.clone(),
                            dmk: item.dmk.unwrap_or(config.dmk),
                        });
                    }
                }
            }
        }

        // 展开 subtask
        let mut expand_subtasks: BTreeMap<u32, SubtaskItem> = config
            .subtasks
            .iter()
            .map(|(&id, &policy)| {
                (
                    id,
                    SubtaskItem {
                        items: vec![],
                        max_score: 0,
                        policy,
                    },
                )
            })
            .collect();

        for (idx, data) in expand_data.iter().enumerate() {
            if let Some(subtask) = expand_subtasks.get_mut(&data.subtask) {
                subtask.items.push(idx);
            } else {
                ctx.emit_warn(format!(
                    "数据点 {} 中发现了无效的 Subtask ID {}",
                    data.id.to_string().cyan(),
                    data.subtask.to_string().cyan()
                ));
            }
        }

        // 计算每个 subtask 的最大分值
        for subtask in expand_subtasks.values_mut() {
            subtask.max_score = match subtask.policy {
                ScorePolicy::Max => subtask
                    .items
                    .iter()
                    .map(|&i| expand_data[i].score)
                    .max()
                    .unwrap_or(0),
                ScorePolicy::Min => subtask
                    .items
                    .iter()
                    .map(|&i| expand_data[i].score)
                    .min()
                    .unwrap_or(0),
                ScorePolicy::Sum => subtask.items.iter().map(|&i| expand_data[i].score).sum(),
            };
        }

        if config.problem_type == ProblemType::Interactive && config.interactive.is_none() {
            bail!("交互题目需要配置交互 (interactive)");
        }

        config.path = config_path
            .parent()
            .context("无法获取配置文件父目录")?
            .into();
        config.runtime.samples = expand_samples;
        config.runtime.inherited_data = inherited_data;
        config.runtime.data = expand_data;
        config.runtime.subtasks = expand_subtasks;

        Ok(config)
    }

    /// 将本层级配置序列化为 `conf.json` 的 JSON 文本，仅含文件视图字段
    ///
    /// 序列化失败时返回 `Err`。
    pub fn save(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&AsSerde::<
            ProblemConfig,
            FileView,
        >::new(self.clone()))?)
    }
}

/// 生成器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GeneratorConfig {
    /// 生成器源文件路径（相对于题目目录）
    #[serde(rename = "gen")]
    pub source: String,
    /// 依赖文件列表（相对于题目目录）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    /// 生成输入后是否进行校验
    #[serde(default)]
    pub validate: bool,
}

/// 生成器配置对（data / sample）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GeneratorConfigPair {
    /// 正式数据生成器
    pub data: GeneratorConfig,
    /// 样例数据生成器，为 null 时使用 data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<GeneratorConfig>,
}

/// Checker 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CheckerConfig {
    /// Checker 源文件路径（相对于题目目录）
    pub source: String,
    /// 依赖文件列表（相对于题目目录）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
}

/// Checker 配置对（data / sample）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CheckerConfigPair {
    /// 正式数据 Checker
    pub data: CheckerConfig,
    /// 样例数据 Checker，为 null 时使用 data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<CheckerConfig>,
}

/// Validator 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ValidatorConfig {
    /// Validator 源文件路径（相对于题目目录）
    pub source: String,
    /// 依赖文件列表（相对于题目目录）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
}

/// Validator 配置对（data / sample）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ValidatorConfigPair {
    /// 正式数据 Validator
    pub data: ValidatorConfig,
    /// 样例数据 Validator，为 null 时使用 data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<ValidatorConfig>,
}

/// 样例配置（文件格式，静态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    /// 样例编号
    pub id: u32,
    /// 输入文件，默认为 `{id}.in`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// 输出文件，默认为 `{id}.ans`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// 原始参数（来自配置文件）
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub args: IndexMap<String, Arg>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

impl SampleItem {
    pub fn input_path(&self) -> String {
        self.input
            .clone()
            .unwrap_or_else(|| format!("{}.in", self.id))
    }
    pub fn output_path(&self) -> String {
        self.output
            .clone()
            .unwrap_or_else(|| format!("{}.ans", self.id))
    }
}

/// 样例配置（运行时，解析路径并合并参数后的结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedSampleItem {
    /// 样例编号
    pub id: u32,
    /// 输入文件
    pub input: String,
    /// 输出文件
    pub output: String,
    /// 参数（继承全局参数后的运行时结果）
    pub args: IndexMap<String, Arg>,
    /// 数据生成行为
    pub dmk: DmkConfig,
}

/// 数据条目：单个测试点或一组测试点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataItem {
    /// 单个对象
    Single(SingleDataItem),
    /// 组合对象
    Bundle(BundleDataItem),
}

/// 单个测试点的原始配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleDataItem {
    /// 测试点编号
    pub id: u32,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号，默认为 0
    #[serde(default)]
    pub subtask: u32,
    /// 输入文件，默认为 `{id}.in`
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input: Option<String>,
    /// 输出文件，默认为 `{id}.ans`
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<String>,
    /// 原始参数（来自配置文件）
    #[serde(rename = "args")]
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub orig_args: IndexMap<String, Arg>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

/// 一组测试点的原始配置，加载时按 `id` 展开为多个测试点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDataItem {
    /// 测试点编号列表，会展开为多个测试点
    pub id: Vec<i32>,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号，默认为 0
    #[serde(default)]
    pub subtask: u32,
    /// 原始参数（来自配置文件）
    #[serde(rename = "args")]
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub orig_args: IndexMap<String, Arg>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

/// 单个测试点的运行时展开结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedDataItem {
    /// 测试点编号
    pub id: u32,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    pub subtask: u32,
    /// 输入文件
    pub input: String,
    /// 输出文件
    pub output: String,
    /// 原始参数（来自配置文件）
    pub orig_args: IndexMap<String, Arg>,
    /// 参数（继承全局参数后的运行时结果）
    pub args: IndexMap<String, Arg>,
    /// 数据生成行为
    pub dmk: DmkConfig,
}

/// 展开后的 Subtask 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskItem {
    /// 数据点在 [`ProblemRuntime::data`] 中的下标
    pub items: Vec<usize>,
    /// 该 Subtask 的满分，按 [`SubtaskItem::policy`] 由成员数据点分值计算
    pub max_score: u32,
    /// 评分策略
    pub policy: ScorePolicy,
}

/// 评测用例：期望得分条件与对应的题解代码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// 期望得分条件
    pub expected: ExpectedScore,
    /// 题解代码路径
    pub path: String,
}

/// 期望得分条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedScore {
    /// 单个条件，如 `">= 60"`
    Single(String),
    /// 多个条件，如 `[">= 60", "< 90"]`
    Multiple(Vec<String>),
}

/// Subtask 得分的聚合方式
#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum ScorePolicy {
    /// 子任务得分取各测试点得分之和
    Sum,
    /// 子任务得分取各测试点中的最高分
    Max,
    /// 子任务得分取各测试点中的最低分
    Min,
}

/// 题目类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProblemType {
    /// 传统型
    Program,
    /// 提交答案型
    Output,
    /// 交互型
    Interactive,
}

/// 交互题配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveConfig {
    /// 交互库路径
    pub grader: String,
    /// 交互库头文件路径
    pub header: String,
    /// 样例交互库路径
    pub sample_grader: Option<String>,
    /// Dmk 交互库路径
    pub dmk_grader: Option<String>,
}

/// 数据生成行为
#[derive(Debug, Clone, Serialize, Deserialize, Copy, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DmkConfig {
    /// 忽略
    Skip,
    /// 只生成输入
    Input,
    /// 只生成输出
    Output,
    /// 启用
    On,
}
