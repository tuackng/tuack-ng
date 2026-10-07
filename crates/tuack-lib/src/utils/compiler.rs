use std::time::Duration;

use crate::data::Reader;
use crate::prelude::*;

/// 运行器元信息
pub struct RunnerManifest {
    /// 是否支持交互式题目
    pub interactive: bool,
}

/// 进程资源限制
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// 时间限制，`None` 表示不限。
    pub time_limit: Option<Duration>,
    /// 内存限制（字节），`None` 表示不限。
    pub memory_limit: Option<u64>,
}

impl ResourceLimits {
    pub fn unlimited() -> Self {
        Self {
            time_limit: None,
            memory_limit: None,
        }
    }

    pub fn new(time_limit: Duration, memory_limit: u64) -> Self {
        Self {
            time_limit: Some(time_limit),
            memory_limit: Some(memory_limit),
        }
    }
}

/// 进程结束状态
#[derive(Debug)]
pub enum RunStatus {
    Success,
    /// 非零退出，携带退出码
    NonZeroExit(i32),
    TimeLimitExceeded,
    MemoryLimitExceeded,
    /// 运行器内部错误，如进程监控失败
    InternalError(anyhow::Error),
}

/// [`Runner::execute`] 的返回结果
pub struct RunResult {
    pub status: RunStatus,
    /// 运行时间，TLE/MLE 时为 `None`。
    pub time: Option<Duration>,
    /// 峰值内存（字节）；运行器内部错误时为 `None`。
    pub memory: Option<u64>,
    /// 程序输出流；`None` 表示输出文件不存在（文件 IO 程序未写输出）。
    pub output: Option<Box<dyn Reader>>,
    /// 程序 stderr
    pub stderr: Vec<u8>,
}

/// IO 模式
#[derive(Debug, Clone)]
pub enum IoMode {
    /// 标准 IO
    Stdio,
    /// 文件 IO
    File {
        /// 输入文件名
        input_name: String,
        /// 输出文件名
        output_name: String,
    },
}

/// 一次运行的输入
pub struct RunSpec {
    /// 资源限制
    pub limits: ResourceLimits,
    /// IO 模式
    pub io_mode: IoMode,
    /// 输入内容：标准 IO 写入其标准输入，文件 IO 写入其输入文件
    pub input: Box<dyn Reader>,
}

/// 运行器：编译 + 资源限制执行。
pub trait Runner: Send {
    /// 获取运行器元数据
    fn manifest(&self) -> RunnerManifest;

    /// 运行前准备（如编译）；须先于 `execute` 调用
    ///
    /// # Errors
    ///
    /// 准备失败时返回 `Err`：临时目录创建或源文件拷贝失败、编译命令构造失败、
    /// 编译器进程无法启动，或编译以非零码退出（编译错误输出在错误消息里）。
    fn prepare(&mut self) -> Result<()>;

    /// 启用交互模式：指定 grader 与头文件，须在 `prepare` 前调用
    ///
    /// # Panics
    ///
    /// 运行器不支持交互式题目（[`RunnerManifest::interactive`] 为 `false`）时
    /// panic；调用方须先经 [`Runner::manifest`] 确认支持。
    fn set_interactive(&mut self, grader_file: &Path, header_file: &Path) -> Result<()>;

    /// 运行结束后的清理；调用后不保证立即释放磁盘占用，实际回收由实现决定。
    fn cleanup(&mut self) -> Result<()>;

    /// 按 `spec` 执行一次程序。
    ///
    /// # Errors
    ///
    /// 运行命令模板渲染或解析失败、程序无法启动（可执行文件不存在、spawn 失败），或
    /// 执行期间的 IO 失败时返回 `Err`；超时与超内存不算错误，以
    /// [`RunStatus::TimeLimitExceeded`] / [`RunStatus::MemoryLimitExceeded`] 由返回值报告。
    fn execute(&mut self, spec: RunSpec) -> Result<RunResult>;
}
