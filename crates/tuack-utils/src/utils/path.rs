//! 路径安全校验：把相对路径约束在可信基准目录内。

use std::path::Component;

use path_clean::PathClean;

use crate::prelude::*;

/// 将 `rel` 解析为 `base` 下的绝对路径，并确认它不会逃出 `base`。
///
/// 词法上拒绝绝对路径与 `..` 越界；文件系统上逐段拒绝符号链接。
/// `base` 及其祖先视为可信边界，只检查 `base` 以下的分量。目标路径可以不存在。
///
/// 只提供路径检查，不保证检查与后续打开之间路径未被替换（TOCTOU）。需要相对路径时用
/// [`resolve_relative_within`]。
///
/// # Errors
///
/// `rel` 为绝对路径、规范化后越出 `base`、含符号链接，或检查路径时遇到除
/// [`std::io::ErrorKind::NotFound`] 以外的文件系统错误时返回 `Err`。
pub fn resolve_within(base: &Path, rel: &Path) -> Result<PathBuf> {
    let base = base.clean();
    let rel = resolve_relative_within(&base, rel)?;
    Ok(base.join(rel))
}

/// 将 `rel` 校验为位于 `base` 内的相对路径。
///
/// 与 [`resolve_within`] 做同样的词法、越界和符号链接检查，但返回相对 `base` 的路径，
/// 不再拼接 `base`。`base` 及其祖先视为可信边界；目标路径可以不存在。
///
/// 只提供路径检查，不保证检查与后续打开之间路径未被替换（TOCTOU）。
///
/// # Errors
///
/// `rel` 为绝对路径、规范化后越出 `base`、含符号链接，或检查路径时遇到除
/// [`std::io::ErrorKind::NotFound`] 以外的文件系统错误时返回 `Err`。
pub fn resolve_relative_within(base: &Path, rel: &Path) -> Result<PathBuf> {
    let base = base.clean();
    let rel = normalize_relative(&base, rel)?;
    reject_symlink_components(&base, &rel)?;
    Ok(rel)
}

/// 规范化 `rel` 并确认它位于 `base` 内，返回相对 `base` 的路径。
fn normalize_relative(base: &Path, rel: &Path) -> Result<PathBuf> {
    let rel = rel.clean();
    if rel
        .components()
        .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        bail!("路径必须为相对路径");
    }
    let dest = base.join(&rel).clean();
    dest.strip_prefix(base)
        .map(Path::to_path_buf)
        .map_err(|_| anyhow!("路径越出基准目录"))
}

/// 从 `base` 开始逐段检查 `rel` 的每个分量，不跟随符号链接。
fn reject_symlink_components(base: &Path, rel: &Path) -> Result<()> {
    let mut current = base.to_path_buf();
    for component in rel.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("路径含符号链接（{}）", current.display());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("检查路径失败（{}）", current.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_path_and_rejects_escape() {
        let base = Path::new("/tmp/base");
        assert_eq!(
            resolve_within(base, Path::new("a/b")).unwrap(),
            base.join("a/b")
        );
        assert_eq!(
            resolve_relative_within(base, Path::new("a/b")).unwrap(),
            Path::new("a/b")
        );
        assert!(resolve_within(base, Path::new("/abs")).is_err());
        assert!(resolve_within(base, Path::new("../x")).is_err());
        assert!(resolve_within(base, Path::new("a/../../x")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_components() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let outside = tempfile::tempdir().unwrap();

        assert!(resolve_within(base, Path::new("missing/file")).is_ok());

        std::fs::write(outside.path().join("secret"), b"x").unwrap();
        symlink(outside.path().join("secret"), base.join("link")).unwrap();
        assert!(resolve_within(base, Path::new("link")).is_err());

        symlink(outside.path().join("missing"), base.join("dangling")).unwrap();
        assert!(resolve_within(base, Path::new("dangling")).is_err());

        symlink(outside.path(), base.join("dir")).unwrap();
        assert!(resolve_within(base, Path::new("dir/x")).is_err());
    }
}
