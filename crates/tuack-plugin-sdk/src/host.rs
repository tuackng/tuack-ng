//! 宿主函数导入与封装。

use std::io::Read;

use extism_pdk::{Json, Msgpack, host_fn};
use tuack_lib::plugin::{AssetChunk, AssetError, CommandError, CommandResult, PathError};

/// 声明由宿主提供的 host 函数。
///
/// # Errors
///
/// 契约内的失败以 payload 的 `Err` 侧返回，按函数分属 [`AssetError`]、[`CommandError`] 或
/// [`PathError`]；内存读写与宿主上下文层面的错误则以
/// [`extism_pdk::Error`](crate::extism_pdk::Error) 结束整个插件调用。
///
/// # Safety
///
/// 调用这些导入函数要求宿主已注册同名 host 函数，且两侧 ABI（参数与返回值的内存布局、
/// `Json`/`Msgpack` 编解码约定）完全一致；不满足时会在跨边界读写处崩溃或读到错乱数据。
/// 本模块的封装（[`AssetReader`]、[`command`]、[`get_path`]）依赖该前提，绕过封装直接调用
/// 需调用方自行担保。
#[host_fn("extism:host/user")]
unsafe extern "ExtismHost" {
    pub fn asset_open(problem_idx: u64, url: String) -> Json<Result<u64, AssetError>>;
    /// 资产块按 msgpack 字节串通道传回。
    pub fn asset_read(asset_id: u64, len: u64) -> Msgpack<Result<AssetChunk, AssetError>>;
    pub fn asset_close(asset_id: u64);
    pub fn asset_copy(asset_id: u64, dest: String) -> Json<Result<(), AssetError>>;
    /// 命令的 `stdout`/`stderr` 按 msgpack 字节串通道传回。
    pub fn run_command(
        program: String,
        args: Json<Vec<String>>,
        cwd: Json<String>,
    ) -> Msgpack<Result<CommandResult, CommandError>>;
    pub fn plugin_log(level: i32, msg: String);
    pub fn host_get_path(path: String) -> Json<Result<String, PathError>>;
}

/// 题目资源流：按 [`Read`] 逐块读取题目的 `url` 资源。
///
/// 注意：此流与宿主持有的流共享当前位置。若先 `read` 到中间或末尾，再把本句柄
/// 放进 [`OutputFile`](crate::OutputFile) 返回，宿主取流落盘时会从当前位置继续拷贝，
/// 产物将只含剩余部分（甚至为空）。要整份拷贝，请勿提前 `read`。
pub struct AssetReader {
    id: u64,
}

impl AssetReader {
    /// 打开第 `problem_idx` 题的资产 `url`，返回可读流。
    ///
    /// # Errors
    ///
    /// 本次调用未提供题目资源时返回 [`AssetError::Unavailable`]；题号未登记或资源路径越界
    /// 时返回 [`AssetError::Load`]（含底层资源加载的整条错误链）；宿主侧协议错误
    /// 包装为 [`AssetError::Internal`]。
    pub fn open(problem_idx: u64, url: &str) -> Result<Self, AssetError> {
        // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
        let outer = unsafe { asset_open(problem_idx, url.to_string()) }
            .map_err(|e| AssetError::Internal(format!("{e:#}")))?;
        Ok(Self { id: outer.0? })
    }

    /// 请求宿主把资产流从当前位置直接拷贝到目标路径（如 `/tmp/xxx`），避免逐块读取。
    ///
    /// 只写 WASI 文件系统，不参与产物回传；作为产物请直接把本流放进 `OutputFile`
    /// （SDK 会转成 [`OutputSpec::Asset`](crate::OutputSpec::Asset)）。
    ///
    /// # Errors
    ///
    /// 目标路径越出工作区或指向只读资源目录时返回 [`AssetError::InvalidPath`]；句柄无效时
    /// 返回 [`AssetError::InvalidHandle`]；创建目标文件或拷贝字节失败时返回
    /// [`AssetError::Io`]；宿主侧协议错误包装为 [`AssetError::Internal`]。
    pub fn copy_to_host(&self, dest: &str) -> Result<(), AssetError> {
        // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
        let outer = unsafe { asset_copy(self.id, dest.to_string()) }
            .map_err(|e| AssetError::Internal(format!("{e:#}")))?;
        outer.0
    }

    /// 消耗自身，交出资产句柄（不再于 Drop 时关闭），供回传 `OutputSpec::Asset` 使用。
    pub(crate) fn into_id(self) -> u64 {
        std::mem::ManuallyDrop::new(self).id
    }
}

impl Read for AssetReader {
    /// 单次调用最多回传 1 MiB，超出部分留给后续读取；失败统一转成
    /// [`std::io::Error::other`]。
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
        let outer = unsafe { asset_read(self.id, buf.len() as u64) }
            .map_err(|e| std::io::Error::other(format!("{e:#}")))?;
        let data = outer.0.map_err(|e| std::io::Error::other(e.to_string()))?.0;
        if data.is_empty() {
            return Ok(0);
        }
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }
}

impl Drop for AssetReader {
    fn drop(&mut self) {
        // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
        unsafe {
            let _ = asset_close(self.id);
        }
    }
}

/// 执行外部命令（宿主侧执行，插件拿回退出码与输出）。
///
/// `program` 为可执行文件名/路径，`args` 为其余参数。`cwd` 为工作目录，须传 [`get_path`] 返回的
/// 宿主路径（工作区根为 `get_path("/")`）。
///
/// 不建议用此命令取回大输出：`stdout`/`stderr` 会整份回传，优先让命令把结果
/// 写入文件（shell 重定向或命令自身的输出文件参数）。
///
/// # Errors
///
/// 命令未能执行时返回 [`CommandError`]：不在组件白名单内（[`CommandError::NotAllowed`]）、
/// `cwd` 非绝对路径或越出工作区（[`CommandError::InvalidCwd`]）、进程未能启动
/// （[`CommandError::SpawnFailed`]）；宿主侧协议错误包装为
/// [`CommandError::Internal`]。命令完成执行时返回 `Ok(CommandResult)`，无论退出码为何。
pub fn command(program: &str, args: &[&str], cwd: &str) -> Result<CommandResult, CommandError> {
    let args = Json(args.iter().map(|s| s.to_string()).collect::<Vec<String>>());
    let cwd = Json(cwd.to_string());
    // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
    let outer = unsafe { run_command(program.to_string(), args, cwd) }
        .map_err(|e| CommandError::Internal(format!("{e:#}")))?;
    outer.0
}

/// 获取 WASI 工作区路径对应的宿主真实文件系统路径（如 `/out` -> 宿主产物目录）。
///
/// 仅覆盖可写工作区（`/` / `/tmp` / `/out`）；只读资源目录 `/assets` 只能在插件内
/// 用 WASI 文件 API 直接读取，不经此函数。
///
/// # Errors
///
/// 路径越出工作区、经软链逃逸、非 UTF-8 或指向只读资源目录时返回 [`PathError::Invalid`]；
/// 宿主侧协议错误包装为 [`PathError::Internal`]。
pub fn get_path(path: &str) -> Result<String, PathError> {
    // SAFETY: 该函数由宿主按上面 extern 块声明的签名注册
    let outer = unsafe { host_get_path(path.to_string()) }
        .map_err(|e| PathError::Internal(format!("{e:#}")))?;
    outer.0
}
