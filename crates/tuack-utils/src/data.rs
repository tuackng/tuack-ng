use std::io;
use std::path::PathBuf;

use crate::prelude::*;
use crate::utils::resolve_within;
use tuack_config::{DmkConfig, ExpandedDataItem, ExpandedSampleItem, ProblemConfig};
use tuack_lib::data::{Data, DataMut, DmkItem, Reader};
use tuack_lib::utils::testlib::Arg;

/// 构造正式数据的 [`FsTestData`] 列表（从 `data/` 读取）。
///
/// # Errors
///
/// 数据点路径为绝对路径、越出 `data/`、含符号链接时返回 `Err`。
pub fn problem_test_data(problem: &ProblemConfig) -> Result<Vec<FsTestData<'_>>> {
    problem
        .runtime
        .data
        .iter()
        .map(|item| FsTestData::from_data(problem.path.join("data"), item))
        .collect()
}

/// 构造样例数据的 [`FsTestData`] 列表（从 `sample/` 读取）。
///
/// # Errors
///
/// 样例路径为绝对路径、越出 `sample/`、含符号链接时返回 `Err`。
pub fn problem_sample_data(problem: &ProblemConfig) -> Result<Vec<FsTestData<'_>>> {
    problem
        .runtime
        .samples
        .iter()
        .map(|item| FsTestData::from_sample(problem.path.join("sample"), item))
        .collect()
}

/// 被包装的数据来源引用
#[derive(Clone)]
enum TestItemRef<'a> {
    Data(&'a ExpandedDataItem),
    Sample(&'a ExpandedSampleItem),
}

/// 从文件系统读取的测试数据，持有已解析的输入输出路径与对配置项的引用
#[derive(Clone)]
pub struct FsTestData<'a> {
    input: PathBuf,
    output: PathBuf,
    item: TestItemRef<'a>,
}

impl<'a> FsTestData<'a> {
    /// 从正式数据项构造；输入输出路径须在 `base_dir` 内。
    ///
    /// # Errors
    ///
    /// 输入或输出路径为绝对路径、越出 `base_dir`、含符号链接时返回 `Err`。
    pub fn from_data(base_dir: PathBuf, item: &'a ExpandedDataItem) -> Result<Self> {
        let input = resolve_within(&base_dir, Path::new(&item.input))
            .with_context(|| format!("无法解析数据点 {} 的输入文件 {}", item.id, item.input))?;
        let output = resolve_within(&base_dir, Path::new(&item.output))
            .with_context(|| format!("无法解析数据点 {} 的输出文件 {}", item.id, item.output))?;
        Ok(Self {
            input,
            output,
            item: TestItemRef::Data(item),
        })
    }

    /// 从样例项构造；输入输出路径须在 `base_dir` 内。
    ///
    /// # Errors
    ///
    /// 输入或输出路径为绝对路径、越出 `base_dir`、含符号链接时返回 `Err`。
    pub fn from_sample(base_dir: PathBuf, item: &'a ExpandedSampleItem) -> Result<Self> {
        let input = resolve_within(&base_dir, Path::new(&item.input))
            .with_context(|| format!("无法解析样例 {} 的输入文件 {}", item.id, item.input))?;
        let output = resolve_within(&base_dir, Path::new(&item.output))
            .with_context(|| format!("无法解析样例 {} 的输出文件 {}", item.id, item.output))?;
        Ok(Self {
            input,
            output,
            item: TestItemRef::Sample(item),
        })
    }

    /// 该测试点的生成行为
    pub fn dmk(&self) -> DmkConfig {
        match &self.item {
            TestItemRef::Data(item) => item.dmk,
            TestItemRef::Sample(item) => item.dmk,
        }
    }

    /// 该测试点是否生成输入
    pub fn gen_input(&self) -> bool {
        matches!(self.dmk(), DmkConfig::Input | DmkConfig::On)
    }

    /// 该测试点是否生成输出
    pub fn gen_output(&self) -> bool {
        matches!(self.dmk(), DmkConfig::Output | DmkConfig::On)
    }

    /// 输入文件的完整路径
    pub fn input_path(&self) -> &Path {
        &self.input
    }

    /// 输出文件的完整路径
    pub fn output_path(&self) -> &Path {
        &self.output
    }

    /// 该测试点的满分（样例约定每点 1 分）
    pub fn full_score(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.score,
            TestItemRef::Sample(_) => 1,
        }
    }

    /// 该测试点所属子任务（样例约定为 0）
    pub fn subtask(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.subtask,
            TestItemRef::Sample(_) => 0,
        }
    }

    /// 该测试点编号
    pub fn id(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.id,
            TestItemRef::Sample(item) => item.id,
        }
    }
}

impl Data for FsTestData<'_> {
    fn input(&self) -> io::Result<Box<dyn Reader>> {
        let f = std::fs::File::open(&self.input)?;
        Ok(Box::new(f))
    }

    fn answer(&self) -> io::Result<Box<dyn Reader>> {
        let f = std::fs::File::open(&self.output)?;
        Ok(Box::new(f))
    }
}

impl DataMut for FsTestData<'_> {
    fn write_input(&self, mut input: Box<dyn Reader>) -> Result<()> {
        let mut f = std::fs::File::create(self.input_path())?;
        std::io::copy(&mut *input, &mut f)?;
        Ok(())
    }

    fn write_output(&self, mut output: Box<dyn Reader>) -> Result<()> {
        let mut f = std::fs::File::create(self.output_path())?;
        std::io::copy(&mut *output, &mut f)?;
        Ok(())
    }
}

impl DmkItem for FsTestData<'_> {
    fn args(&self) -> &IndexMap<String, Arg> {
        match &self.item {
            TestItemRef::Data(item) => &item.args,
            TestItemRef::Sample(item) => &item.args,
        }
    }
}
