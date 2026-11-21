use std::time::Duration;

use bytesize::ByteSize;

use crate::data::Data;
use crate::prelude::*;
use crate::utils::compiler::{IoMode, ResourceLimits, RunSpec, RunStatus, Runner};
use crate::utils::testlib::{Checker, JudgeResult};

/// 测试点评测状态
#[derive(Debug)]
pub enum TestCaseStatus {
    AC,
    WA,
    RE,
    TLE,
    MLE,
    /// 未归类错误：本次运行无法完成或校验器失败
    UKE,
    /// 文件错误：输出文件不存在
    FE,
    /// 部分正确，携带 0-100 的比例分
    PC(f64),
}

/// 单个测试点评测结果
#[derive(Debug)]
pub struct TestCaseResult {
    pub status: TestCaseStatus,
    /// 归一化得分，`AC=1.0`、`PC=p/100`、其余 `0.0`
    pub score: f64,
    /// 实测运行时间；本次运行无法完成或读取输入/答案失败时无值
    pub time: Option<Duration>,
    /// 峰值内存；未采到读数、本次运行无法完成或读取输入/答案失败时无值
    pub memory: Option<ByteSize>,
    /// 补充说明：校验器消息或失败原因
    pub message: Option<String>,
}

/// 测试会话参数
#[derive(Debug, Clone)]
pub struct TaskParams {
    pub io_mode: IoMode,
    pub time_limit: Duration,
    /// 内存限制（字节）
    pub memory_limit: ByteSize,
}

/// 测试会话
pub struct TestSession<'a> {
    runner: &'a mut dyn Runner,
    checker: &'a dyn Checker,
    params: TaskParams,
}

/// 构造一个 UKE 结果（例如输入/答案文件读取失败）。
fn uke_result(message: String) -> TestCaseResult {
    TestCaseResult {
        status: TestCaseStatus::UKE,
        score: 0.0,
        time: None,
        memory: None,
        message: Some(message),
    }
}

impl<'a> TestSession<'a> {
    /// 创建会话；须保证 [`Runner`] 与 [`Checker`] 已完成 `prepare`
    /// （[`Runner::prepare`]、[`Checker::prepare`]）。
    pub fn new(runner: &'a mut dyn Runner, checker: &'a dyn Checker, params: TaskParams) -> Self {
        Self {
            runner,
            checker,
            params,
        }
    }

    /// 评测单个测试点：构造本次运行的输入 -> 执行 -> 校验 -> 返回结果
    ///
    /// 判定类失败都归一化为 [`TestCaseResult`]，不返回 `Err`。本次运行无法完成
    /// （[`RunStatus::Incomplete`]）统一记 UKE，含评测环境故障。
    ///
    /// # Errors
    ///
    /// 运行器自身问题返回 [`RunnerError`](crate::utils::compiler::RunnerError)。
    pub fn judge(&mut self, data: &dyn Data) -> Result<TestCaseResult> {
        let input = match data.input() {
            Ok(i) => i,
            Err(e) => return Ok(uke_result(format!("读取输入失败：{e}"))),
        };
        let run = self.runner.execute(RunSpec {
            limits: ResourceLimits::new(self.params.time_limit, self.params.memory_limit.as_u64()),
            io_mode: self.params.io_mode.clone(),
            input,
        })?;

        let (status, score, message) = match (run.status, run.output) {
            (RunStatus::Success, None) => {
                (TestCaseStatus::FE, 0.0, Some("未找到输出文件".to_string()))
            }
            (RunStatus::Success, Some(mut output)) => {
                let mut input = match data.input() {
                    Ok(i) => i,
                    Err(e) => return Ok(uke_result(format!("读取输入失败：{e}"))),
                };
                let mut answer = match data.answer() {
                    Ok(a) => a,
                    Err(e) => return Ok(uke_result(format!("读取答案失败：{e}"))),
                };
                match self.checker.validate(&mut input, &mut output, &mut answer) {
                    Ok((JudgeResult::Accepted, msg)) => (TestCaseStatus::AC, 1.0, Some(msg)),
                    Ok((JudgeResult::WrongAnswer, msg)) => (TestCaseStatus::WA, 0.0, Some(msg)),
                    Ok((JudgeResult::PresentationError, msg)) => {
                        (TestCaseStatus::WA, 0.0, Some(msg))
                    }
                    Ok((JudgeResult::Score(p), msg)) => (
                        TestCaseStatus::PC(p),
                        (p / 100.0).clamp(0.0, 1.0),
                        Some(msg),
                    ),
                    Ok((JudgeResult::Fail, msg)) => (TestCaseStatus::UKE, 0.0, Some(msg)),
                    Err(e) => (TestCaseStatus::UKE, 0.0, Some(format!("{e:#}"))),
                }
            }
            (RunStatus::NonZeroExit(_), _) => (TestCaseStatus::RE, 0.0, None),
            (RunStatus::TimeLimitExceeded, _) => (TestCaseStatus::TLE, 0.0, None),
            (RunStatus::MemoryLimitExceeded, _) => (TestCaseStatus::MLE, 0.0, None),
            (RunStatus::Incomplete(e), _) => (TestCaseStatus::UKE, 0.0, Some(format!("{e:#}"))),
        };

        Ok(TestCaseResult {
            status,
            score,
            time: run.time,
            memory: run.memory.map(ByteSize),
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;
    use std::time::Duration;

    use bytesize::ByteSize;

    use super::{TaskParams, TestCaseStatus, TestSession};
    use crate::data::{Data, Reader};
    use crate::prelude::*;
    use crate::utils::compiler::{
        IoMode, MonitorError, RunError, RunResult, RunSpec, RunStatus, Runner, RunnerError,
        RunnerManifest,
    };
    use crate::utils::testlib::{Checker, JudgeResult};

    /// 空数据源：输入/答案都是空流
    struct EmptyData;

    impl Data for EmptyData {
        fn input(&self) -> io::Result<Box<dyn Reader>> {
            Ok(Box::new(io::empty()))
        }

        fn answer(&self) -> io::Result<Box<dyn Reader>> {
            Ok(Box::new(io::empty()))
        }
    }

    /// 执行总是失败的运行器：`monitor` 为真时返回监督器故障，否则返回本次运行无法完成
    struct FaultRunner {
        monitor: bool,
    }

    impl Runner for FaultRunner {
        fn manifest(&self) -> RunnerManifest {
            RunnerManifest { interactive: false }
        }

        fn prepare(&mut self) -> Result<()> {
            Ok(())
        }

        fn set_interactive(&mut self, _grader_file: &Path, _header_file: &Path) -> Result<()> {
            Ok(())
        }

        fn cleanup(&mut self) -> Result<()> {
            Ok(())
        }

        fn execute(&mut self, _spec: RunSpec) -> Result<RunResult, RunnerError> {
            if self.monitor {
                Err(RunnerError::Monitor(MonitorError::MissingPid))
            } else {
                Ok(RunResult {
                    status: RunStatus::Incomplete(RunError::Input(io::Error::other(
                        "写入输入失败",
                    ))),
                    time: None,
                    memory: None,
                    output: None,
                    stderr: None,
                })
            }
        }
    }

    /// 运行失败时不应被调用的校验器
    struct UnusedChecker;

    impl Checker for UnusedChecker {
        fn prepare(&mut self) -> Result<()> {
            Ok(())
        }

        fn validate(
            &self,
            _input: &mut dyn Reader,
            _output: &mut dyn Reader,
            _answer: &mut dyn Reader,
        ) -> Result<(JudgeResult, String)> {
            unreachable!("运行失败时不应调用校验器")
        }
    }

    fn params() -> TaskParams {
        TaskParams {
            io_mode: IoMode::Stdio,
            time_limit: Duration::from_secs(1),
            memory_limit: ByteSize(1 << 20),
        }
    }

    /// 本次运行无法完成时归一化为 UKE，不上抛
    #[test]
    fn incomplete_is_uke() {
        let mut runner = FaultRunner { monitor: false };
        let checker = UnusedChecker;
        let mut session = TestSession::new(&mut runner, &checker, params());

        let result = session.judge(&EmptyData).expect("运行故障不应上抛");

        assert!(matches!(result.status, TestCaseStatus::UKE));
        assert_eq!(result.score, 0.0);
        assert!(
            result
                .message
                .as_deref()
                .is_some_and(|message| message.contains("注入输入失败")),
            "UKE 消息应携带故障原因（{:?}）",
            result.message
        );
    }

    /// 运行器自身错误上抛，不归一化
    #[test]
    fn runner_error_propagates() {
        let mut runner = FaultRunner { monitor: true };
        let checker = UnusedChecker;
        let mut session = TestSession::new(&mut runner, &checker, params());

        assert!(session.judge(&EmptyData).is_err());
    }
}
