use crate::prelude::*;

/// 模板产出的目标格式
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum TargetType {
    Typst,
    Markdown,
}

/// 模板清单（`<assets_dir>/templates/<name>.json`）
///
/// 三个布尔字段给出模板的渲染默认值，可被工程 day/contest 配置覆盖。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateManifest {
    /// 默认是否包含预测试点
    #[serde(default = "default_use_pretest")]
    pub use_pretest: bool,
    /// 默认是否采用 NOI 风格（按测试点计分，而非子任务）
    #[serde(default = "default_noi_style")]
    pub noi_style: bool,
    /// 默认是否使用文件 IO（否则为标准 IO）
    #[serde(default = "default_file_io")]
    pub file_io: bool,
    pub target: TargetType,
    /// 模板文件清单：相对路径 -> assets store 内的内容文件名（sha256）
    #[serde(default)]
    pub filelist: IndexMap<String, String>,
    /// 渲染前按序应用的内置处理器名；顺序影响变换结果
    #[serde(default)]
    pub processor: Vec<String>,
}

fn default_use_pretest() -> bool {
    false
}

fn default_noi_style() -> bool {
    true
}

fn default_file_io() -> bool {
    true
}
