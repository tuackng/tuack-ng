//! `run_command` 宿主函数的返回契约。

use crate::prelude::*;

/// 外部命令执行结果（宿主暴露给插件的 `run_command` 返回值）。
///
/// `stdout`/`stderr` 按原样过边界（`serde_bytes`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: i32,
    #[serde(with = "serde_bytes")]
    pub stdout: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub stderr: Vec<u8>,
}

/// 命令未能执行的可恢复错误（`run_command` payload 的 `Err` 侧）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum CommandError {
    /// 命令不在组件白名单内
    #[error("插件无权执行命令：{0}")]
    NotAllowed(String),
    /// 进程未能启动（可执行文件不存在、无权限或格式错误）
    #[error("无法启动命令 {program}：{message}")]
    SpawnFailed { program: String, message: String },
    /// 工作目录越界或非法
    #[error("非法工作目录：{0}")]
    InvalidCwd(String),
    /// 协议/内部错误（通常不可恢复）
    #[error("内部错误：{0}")]
    Internal(String),
}
