use std::cmp::max;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::time::{Instant, sleep};

use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::prelude::*;
use tuack_lib::utils::compiler::{ResourceLimits, RunStatus};

/// 监视器专用 runtime：供同步代码短时进入异步以监督子进程。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 返回监视器 runtime；首次调用时惰性构建单线程（`current_thread`）runtime。
pub fn monitor_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("构建监视 runtime 失败")
    })
}

/// 子进程 TLE/MLE 监控器：持有 [`ResourceLimits`]，供
/// [`ProcessSupervisor::supervise_blocking`]/[`ProcessSupervisor::supervise`]
/// 监督子进程并在超限时终止
pub struct ProcessSupervisor {
    limits: ResourceLimits,
}

impl ProcessSupervisor {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    /// 在监视器 runtime 上 `block_on` 一次 [`ProcessSupervisor::supervise`]，供同步调用方使用。
    ///
    /// `cmd` 的 stdin/stdout/stderr 由调用方预设；本函数只负责把它启动为子进程。
    ///
    /// # Panics
    ///
    /// 在异步上下文中调用时 panic（`block_on` 不能在 runtime 内执行）。
    ///
    /// # Errors
    ///
    /// 子进程启动失败，或 [`ProcessSupervisor::supervise`] 失败时返回 `Err`。
    pub fn supervise_blocking(
        self,
        cmd: std::process::Command,
    ) -> Result<(RunStatus, Option<Duration>, Option<u64>)> {
        monitor_runtime().block_on(async move {
            let mut child = tokio::process::Command::from(cmd).spawn()?;
            self.supervise(&mut child).await
        })
    }

    /// 监督一个已 spawn 的子进程，超出时限或内存上限即杀掉；时限判定留 200ms 宽限。
    ///
    /// # Errors
    ///
    /// 取不到子进程 PID（进程已退出）时返回 `Err`；TLE/MLE 经 [`RunStatus`] 返回，不算错误。
    pub async fn supervise(
        self,
        child: &mut tokio::process::Child,
    ) -> Result<(RunStatus, Option<Duration>, Option<u64>)> {
        let pid = child.id().context("无法获取进程 PID")?;
        let start = Instant::now();
        let time_limit = self.limits.time_limit.unwrap_or(Duration::MAX);
        let memory_limit = self.limits.memory_limit.unwrap_or(u64::MAX);
        let peak_memory = Arc::new(Mutex::new(0u64));
        let monitoring_peak = Arc::clone(&peak_memory);

        let result = tokio::select! {
            biased;

            _ = async move {
                let mut sys = System::new();
                let sys_pid = Pid::from_u32(pid);
                loop {
                    sleep(Duration::from_millis(20)).await;
                    if start.elapsed() > time_limit.saturating_add(Duration::from_millis(200)) {
                        return;
                    }
                    sys.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), false);
                    if let Some(process) = sys.process(sys_pid) {
                        let memory = process.memory();
                        let mut peak = monitoring_peak.lock().unwrap();
                        *peak = max(*peak, memory);
                        if memory > memory_limit {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            } => {
                // 监视分支返回即表示命中限制或进程已消失：在此统一杀进程，并按是否超时归类。
                let _ = child.kill().await;
                let final_peak = *peak_memory.lock().unwrap();
                if start.elapsed() > time_limit {
                    info!("进程超时：{}", start.elapsed().as_secs_f64());
                    (RunStatus::TimeLimitExceeded, None, Some(final_peak))
                } else {
                    info!("进程内存超限");
                    (RunStatus::MemoryLimitExceeded, None, Some(final_peak))
                }
            }

            exit_status = child.wait() => {
                let elapsed = start.elapsed();
                let final_peak = *peak_memory.lock().unwrap();

                if elapsed > time_limit {
                    return Ok((RunStatus::TimeLimitExceeded, None, Some(final_peak)));
                }

                if final_peak > memory_limit {
                    return Ok((RunStatus::MemoryLimitExceeded, None, Some(final_peak)));
                }

                info!("进程结束，耗时：{:?}, 峰值内存：{}, 退出码：{:?}",
                    elapsed, final_peak, exit_status.as_ref().ok().and_then(|s| s.code()));
                match exit_status {
                    Ok(status) if status.success() => {
                        (RunStatus::Success, Some(elapsed), Some(final_peak))
                    }
                    Ok(status) => {
                        (RunStatus::NonZeroExit(status.code().unwrap_or(-1)), Some(elapsed), Some(final_peak))
                    }
                    Err(e) => {
                        (RunStatus::InternalError(e.into()), None, None)
                    }
                }
            }
        };

        Ok(result)
    }
}
