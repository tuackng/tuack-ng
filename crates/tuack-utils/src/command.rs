use crate::prelude::*;
use std::process::Command;

/// 解析命令串为 [`Command`]，按 shellwords 规则拆分。
///
/// # Errors
///
/// shellwords 解析失败，或拆分结果为空时返回 `Err`。
pub fn string_to_command(command_str: &str) -> Result<Command> {
    let parts = shellwords::split(command_str)?;

    if parts.is_empty() {
        bail!("Empty command");
    }

    let mut cmd = Command::new(&parts[0]);

    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    Ok(cmd)
}
