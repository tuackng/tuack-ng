//! extism 插件共用的宿主上下文与 host 函数。

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use extism::convert::{Json, Msgpack};
use extism::{CurrentPlugin, UserData, Val};
use path_clean::PathClean;

use tuack_lib::data::Reader;
use tuack_lib::plugin::{
    AssetChunk, AssetError, CommandError, CommandResult, OutputSpec, PathError,
};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

use crate::utils::KeepAliveReader;

use crate::prelude::*;

/// 资产流句柄表：宿主与插件上下文共享，call 结束后宿主仍可取回未消费的流。
pub(crate) type AssetStreams = Arc<Mutex<HashMap<u64, Box<dyn Reader>>>>;

/// 只读资源挂载点：各插件工厂把包的 `asset_dir` 挂到 `with_allowed_path(..., "/assets")`。
///
/// 该挂载点位于工作区之外，`host_get_path`、`run_command` 的 `cwd` 与 `asset_copy` 的目标都不接受。
pub(crate) const READONLY_ASSET_MOUNT: &str = "/assets";

/// 插件调用期间的主机上下文（经 `call_with_host_context` 注入，供 host 函数访问）。
pub(crate) struct PluginContext {
    /// 题目资源提供方；处理器无题目资源时为 `None`
    assets: Option<Box<dyn AssetProvider>>,
    streams: AssetStreams,
    next_id: AtomicU64,
    tmp_dir: PathBuf,
    /// 允许执行的宿主可执行文件白名单
    command: Vec<String>,
}

impl PluginContext {
    pub(crate) fn new(
        assets: Option<Box<dyn AssetProvider>>,
        tmp_dir: PathBuf,
        streams: AssetStreams,
        command: Vec<String>,
    ) -> Self {
        Self {
            assets,
            streams,
            next_id: AtomicU64::new(0),
            tmp_dir,
            command,
        }
    }

    /// 打开第 `problem_idx` 题的资产 `url`，返回句柄。
    fn open_asset(&self, problem_idx: u64, url: &str) -> Result<u64, AssetError> {
        let assets = self.assets.as_ref().ok_or(AssetError::Unavailable)?;
        let stream = assets
            .load(problem_idx, Path::new(url))
            .map_err(|e| AssetError::Load(format!("{e:#}")))?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.streams.lock().unwrap().insert(id, stream);
        Ok(id)
    }

    /// 读取资产的一段，读到 EOF 返回空。
    fn read_asset(&self, asset_id: u64, len: u64) -> Result<Vec<u8>, AssetError> {
        // `len` 由插件传入，钳到上限避免恶意/有 bug 的插件让宿主任意分配。
        const MAX_ASSET_CHUNK: u64 = 1024 * 1024;
        let len = len.min(MAX_ASSET_CHUNK);
        let mut streams = self.streams.lock().unwrap();
        let stream = streams
            .get_mut(&asset_id)
            .ok_or(AssetError::InvalidHandle(asset_id))?;
        let mut buf = vec![0u8; len as usize];
        let n = stream
            .read(&mut buf)
            .map_err(|e| AssetError::Io(e.to_string()))?;
        buf.truncate(n);
        Ok(buf)
    }

    /// 关闭资产句柄。
    fn close_asset(&self, asset_id: u64) {
        self.streams.lock().unwrap().remove(&asset_id);
    }

    /// 把资产流从当前位置直接拷贝到 `dest` 对应的 WASI 文件（不参与产物回传）；
    /// 只读资源挂载不可作为目标。
    fn copy_asset(&self, asset_id: u64, dest: &str) -> Result<(), AssetError> {
        if under_readonly_mount(dest) {
            return Err(AssetError::InvalidPath(format!(
                "只读资源目录不可写入：{dest}"
            )));
        }
        let host_dest = resolve_within(&self.tmp_dir, dest)
            .map_err(|e| AssetError::InvalidPath(format!("{e:#}")))?;
        if let Some(parent) = host_dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AssetError::Io(e.to_string()))?;
        }
        let mut streams = self.streams.lock().unwrap();
        let stream = streams
            .get_mut(&asset_id)
            .ok_or(AssetError::InvalidHandle(asset_id))?;
        let mut f = std::fs::File::create(&host_dest).map_err(|e| AssetError::Io(e.to_string()))?;
        std::io::copy(stream, &mut f).map_err(|e| AssetError::Io(e.to_string()))?;
        Ok(())
    }

    /// 执行宿主命令（受白名单约束）。
    ///
    /// 可恢复失败（白名单、cwd、进程未启动）返回 `Err(CommandError)`；
    /// 命令完成执行（无论退出码为何）则返回 `Ok(CommandResult)`。
    fn run_command(
        &self,
        program: &str,
        args: &[String],
        cwd: &str,
    ) -> Result<CommandResult, CommandError> {
        if !self.command.iter().any(|c| c == "*" || c == program) {
            return Err(CommandError::NotAllowed(program.to_string()));
        }
        let cwd = self
            .resolve_cwd(cwd)
            .map_err(|e| CommandError::InvalidCwd(format!("{e:#}")))?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        cmd.current_dir(cwd);
        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) => {
                return Err(CommandError::SpawnFailed {
                    program: program.to_string(),
                    message: e.to_string(),
                });
            }
        };
        Ok(CommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// 把插件传入的 `cwd` 限定在工作区内：必须是 [`get_path`] 返回的宿主绝对路径，且位于工作区内。
    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf> {
        let path = Path::new(cwd);
        if !path.is_absolute() {
            bail!("工作目录必须是宿主绝对路径：{cwd}");
        }
        let cleaned = path.clean();
        let Ok(rel) = cleaned.strip_prefix(&self.tmp_dir) else {
            bail!("工作目录越出工作区：{cwd}");
        };
        crate::utils::resolve_within(&self.tmp_dir, rel)
    }
}

/// 取当前调用的主机上下文并执行 `f`，返回值原样带出（`f` 返回后 `plugin` 的借用才结束）。
fn with_context<T>(
    plugin: &mut CurrentPlugin,
    f: impl FnOnce(&mut PluginContext) -> T,
) -> Result<T> {
    let ctx = plugin.host_context::<PluginContext>()?;
    Ok(f(ctx))
}

fn asset_open(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let problem_idx: u64 = plugin.memory_get_val(&inputs[0])?;
    let url: String = plugin.memory_get_val(&inputs[1])?;
    let outcome = with_context(plugin, |ctx| ctx.open_asset(problem_idx, &url))?;
    plugin.memory_set_val(&mut outputs[0], Json(outcome))?;
    Ok(())
}

fn asset_read(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let len: u64 = plugin.memory_get_val(&inputs[1])?;
    // 资产块走 msgpack + `AssetChunk` 的字节串通道。
    let outcome = with_context(plugin, |ctx| ctx.read_asset(asset_id, len).map(AssetChunk))?;
    plugin.memory_set_val(&mut outputs[0], Msgpack(outcome))?;
    Ok(())
}

fn asset_close(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    with_context(plugin, |ctx| ctx.close_asset(asset_id))?;
    Ok(())
}

fn asset_copy(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let dest: String = plugin.memory_get_val(&inputs[1])?;
    let outcome = with_context(plugin, |ctx| ctx.copy_asset(asset_id, &dest))?;
    plugin.memory_set_val(&mut outputs[0], Json(outcome))?;
    Ok(())
}

fn run_command(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let program: String = plugin.memory_get_val(&inputs[0])?;
    let args: Json<Vec<String>> = plugin.memory_get_val(&inputs[1])?;
    let cwd: Json<String> = plugin.memory_get_val(&inputs[2])?;
    // 命令输出同样走 msgpack 的字节串通道
    let outcome = with_context(plugin, |ctx| ctx.run_command(&program, &args.0, &cwd.0))?;
    plugin.memory_set_val(&mut outputs[0], Msgpack(outcome))?;
    Ok(())
}

fn get_path(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let path: String = plugin.memory_get_val(&inputs[0])?;
    let outcome = if under_readonly_mount(&path) {
        Err(PathError::Invalid(format!(
            "只读资源目录不可解析为宿主路径：{path}"
        )))
    } else {
        with_context(plugin, |ctx| resolve_workspace_path(&ctx.tmp_dir, &path))?
    };
    plugin.memory_set_val(&mut outputs[0], Json(outcome))?;
    Ok(())
}

/// 构造所有插件共用的 host 函数（日志 + 资产 + 命令 + 路径）。
pub(crate) fn common_imports() -> Vec<extism::Function> {
    vec![
        super::log_import(),
        extism::Function::new(
            "asset_open",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            asset_open,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_read",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            asset_read,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_close",
            [extism::PTR],
            [],
            UserData::default(),
            asset_close,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_copy",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            asset_copy,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "run_command",
            [extism::PTR, extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            run_command,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "host_get_path",
            [extism::PTR],
            [extism::PTR],
            UserData::default(),
            get_path,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
    ]
}

/// 把 `rel`（WASI 风格，可能带前导 `/`）解析到 `base` 下；越出 `base` 或经符号链接逃逸则报错。
fn resolve_within(base: &Path, rel: &str) -> Result<PathBuf> {
    let stripped = rel.strip_prefix('/').unwrap_or(rel);
    crate::utils::resolve_within(base, Path::new(stripped))
}

/// 判断 WASI 风格路径是否落在只读资源挂载下（`/assets` 或 `/assets/...`）。
fn under_readonly_mount(path: &str) -> bool {
    path == READONLY_ASSET_MOUNT
        || path
            .strip_prefix(READONLY_ASSET_MOUNT)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// 把 WASI 风格路径解析到工作区，并转成交给插件的 UTF-8 宿主路径；非 UTF-8 时返回
/// [`PathError::Invalid`]。
fn resolve_workspace_path(base: &Path, rel: &str) -> Result<String, PathError> {
    let resolved = resolve_within(base, rel).map_err(|e| PathError::Invalid(format!("{e:#}")))?;
    resolved.into_os_string().into_string().map_err(|raw| {
        PathError::Invalid(format!("工作区路径非 UTF-8：{}", Path::new(&raw).display()))
    })
}

/// 把回传的 `OutputSpec` 转成宿主 `OutputFile`：资产流从共享句柄表取出（host-to-host），
/// 文件从插件 WASI 工作区 `/out` 打开；`keepalive` 保证临时目录存活到流被消费。
pub(crate) fn specs_to_outputs(
    specs: Vec<OutputSpec>,
    streams: &AssetStreams,
    out_dir: &Path,
    keepalive: Arc<tempfile::TempDir>,
) -> Result<Vec<OutputFile>> {
    let mut guard = streams.lock().unwrap();
    let mut files = Vec::new();
    for spec in specs {
        match spec {
            OutputSpec::Asset { path, asset_id } => {
                let path = crate::utils::normalize_within(out_dir, &path)?;
                let stream = guard
                    .remove(&asset_id)
                    .ok_or(AssetError::InvalidHandle(asset_id))?;
                files.push(OutputFile::File {
                    path,
                    bytes: stream,
                });
            }
            OutputSpec::File { path } => {
                let rel = crate::utils::normalize_within(out_dir, &path)?;
                let dest = out_dir.join(&rel);
                crate::utils::assert_within(out_dir, &dest)?;
                let file = std::fs::File::open(&dest)
                    .with_context(|| format!("打开产物文件失败：{}", dest.display()))?;
                files.push(OutputFile::File {
                    path: rel,
                    bytes: Box::new(KeepAliveReader::new(file, keepalive.clone())),
                });
            }
            OutputSpec::Dir(path) => {
                let path = crate::utils::normalize_within(out_dir, &path)?;
                files.push(OutputFile::Dir(path));
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(dir: &Path) -> PluginContext {
        PluginContext::new(
            None,
            dir.to_path_buf(),
            Arc::new(Mutex::new(HashMap::new())),
            vec!["typst".to_string()],
        )
    }

    /// 桩资源提供方：任意 `idx` 都返回同一段字节；`missing` 触发加载失败。
    struct StubAssets(Vec<u8>);

    impl AssetProvider for StubAssets {
        fn load(&self, _idx: u64, path: &Path) -> Result<Box<dyn Reader>> {
            if path == Path::new("missing") {
                return Err(anyhow::anyhow!("打开资源失败：{}", path.display()));
            }
            Ok(Box::new(std::io::Cursor::new(self.0.clone())))
        }
    }

    fn context_with_assets(dir: &Path, bytes: Vec<u8>) -> PluginContext {
        PluginContext::new(
            Some(Box::new(StubAssets(bytes))),
            dir.to_path_buf(),
            Arc::new(Mutex::new(HashMap::new())),
            Vec::new(),
        )
    }

    #[test]
    fn asset_reads_to_eof_then_closes() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = context_with_assets(tmp.path(), vec![1, 2, 3, 4, 5]);

        let id = ctx.open_asset(0, "a.txt").unwrap();
        assert_eq!(ctx.read_asset(id, 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(ctx.read_asset(id, 3).unwrap(), vec![4, 5]);
        assert_eq!(ctx.read_asset(id, 3).unwrap(), Vec::<u8>::new());

        ctx.close_asset(id);
        assert_eq!(ctx.read_asset(id, 1), Err(AssetError::InvalidHandle(id)));
    }

    #[test]
    fn asset_read_clamps_requested_len() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = context_with_assets(tmp.path(), vec![7; 4]);

        // 未钳制时 `vec![0u8; u64::MAX as usize]` 会直接 abort
        let id = ctx.open_asset(0, "a.txt").unwrap();
        assert_eq!(ctx.read_asset(id, u64::MAX).unwrap(), vec![7; 4]);
    }

    #[test]
    fn asset_open_reports_load_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = context_with_assets(tmp.path(), vec![0]);

        assert!(matches!(
            ctx.open_asset(0, "missing"),
            Err(AssetError::Load(_))
        ));
    }

    #[test]
    fn asset_access_without_provider_is_recoverable() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());

        assert_eq!(ctx.open_asset(0, "a.txt"), Err(AssetError::Unavailable));
    }

    #[test]
    fn copy_asset_writes_into_workspace_and_rejects_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = context_with_assets(tmp.path(), b"hello".to_vec());

        let id = ctx.open_asset(0, "a.txt").unwrap();
        ctx.copy_asset(id, "/tmp/a.txt").unwrap();
        assert_eq!(
            std::fs::read(tmp.path().join("tmp/a.txt")).unwrap(),
            b"hello"
        );

        assert!(matches!(
            ctx.copy_asset(id, "../x"),
            Err(AssetError::InvalidPath(_))
        ));
        // 只读资源挂载：不得把 WASI 视图里的只读路径改道写成工作区内的同名子目录
        assert!(matches!(
            ctx.copy_asset(id, "/assets/a.txt"),
            Err(AssetError::InvalidPath(_))
        ));
        assert!(!tmp.path().join("assets").exists());
        assert!(matches!(
            ctx.copy_asset(999, "y"),
            Err(AssetError::InvalidHandle(999))
        ));
    }

    #[test]
    fn readonly_asset_mount_is_recognized() {
        assert!(under_readonly_mount("/assets"));
        assert!(under_readonly_mount("/assets/img/a.png"));
        assert!(!under_readonly_mount("/assetsx"));
        assert!(!under_readonly_mount("/tmp/assets"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_workspace_path_reports_non_utf8_as_recoverable() {
        use std::os::unix::ffi::OsStrExt;

        let tmp = tempfile::tempdir().unwrap();
        // 工作区根自身含非法 UTF-8（例如 TMPDIR 指向这样的目录）
        let base = tmp.path().join(std::ffi::OsStr::from_bytes(b"ws\xff"));
        std::fs::create_dir(&base).unwrap();

        assert!(matches!(
            resolve_workspace_path(&base, "a.txt"),
            Err(PathError::Invalid(_))
        ));
    }

    #[test]
    fn resolve_cwd_maps_host_path_inside_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());

        // `get_path("/")` 的返回值：工作区根
        assert_eq!(
            ctx.resolve_cwd(&tmp.path().to_string_lossy()).unwrap(),
            tmp.path()
        );
        assert_eq!(
            ctx.resolve_cwd(&tmp.path().join("out/x").to_string_lossy())
                .unwrap(),
            tmp.path().join("out/x")
        );
    }

    #[test]
    fn resolve_cwd_rejects_non_workspace_host_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());

        // 非宿主绝对路径
        assert!(ctx.resolve_cwd("").is_err());
        assert!(ctx.resolve_cwd("out/x").is_err());
        assert!(ctx.resolve_cwd("../x").is_err());
        // 工作区外的宿主路径，含只读资源挂载的 WASI 拼写
        assert!(ctx.resolve_cwd("/tmp").is_err());
        assert!(ctx.resolve_cwd("/etc").is_err());
        assert!(ctx.resolve_cwd("/assets").is_err());
        assert!(ctx.resolve_cwd("/assets/x").is_err());
        assert!(ctx.resolve_cwd("/../../etc").is_err());
    }

    #[test]
    fn run_command_enforces_whitelist() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());
        let cwd = tmp.path().to_string_lossy().to_string();

        assert!(matches!(
            ctx.run_command("sh", &[], &cwd),
            Err(CommandError::NotAllowed(_))
        ));
    }
}
