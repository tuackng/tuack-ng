//! 长任务的进度显示。

use std::time::Duration;

use indicatif::ProgressBar;

use crate::context::gctx;
use crate::prelude::Result;

/// 在长任务上显示转圈进度；任务结束（成功或失败）后清除。
pub fn with_spinner<T>(message: impl Into<String>, task: impl FnOnce() -> Result<T>) -> Result<T> {
    let pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(message.into());

    let result = task();
    pb.finish_and_clear();
    result
}
