//! extism 处理器插件：宿主侧封装。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extism::WasmInput;
use extism::convert::Json;
use extism::{Manifest, Wasm};
use tempfile::TempDir;
use tuack_lib::ren::{ProcessorOutput, RenProcessor};
use tuack_ng_parser::ast::Document;

use crate::plugin::extism::context::{
    AssetStreams, PluginContext, READONLY_ASSET_MOUNT, common_imports,
};
use crate::prelude::*;

/// 一个基于 extism 的处理器插件
pub struct ExtismProcessor {
    plugin: Mutex<extism::Plugin>,
    function: String,
    tmp: Arc<TempDir>,
    command: Vec<String>,
}

impl ExtismProcessor {
    /// 加载 wasm 插件并校验其导出函数存在，同时准备独立的临时工作区
    ///
    /// `asset_dir` 为插件随包资源目录（只读 `/assets`）；`command` 为宿主命令白名单；
    /// 临时工作区映射到 WASI `/`。
    ///
    /// # Errors
    ///
    /// 创建临时目录失败、加载插件失败，或插件未导出 `function` 时返回 `Err`。
    pub fn new(
        wasm: Vec<u8>,
        function: String,
        with_wasi: bool,
        asset_dir: Option<PathBuf>,
        command: Vec<String>,
    ) -> Result<Self> {
        let tmp = Arc::new(
            tempfile::Builder::new()
                .prefix("tuack-ng-proc-")
                .tempdir()
                .context("创建临时目录失败")?,
        );
        let tmp_dir = tmp.path();
        fs::create_dir_all(tmp_dir.join("tmp"))?;

        let mut manifest = Manifest::new([Wasm::data(wasm)])
            .with_allowed_path(tmp_dir.to_string_lossy().to_string(), "/");
        if let Some(asset_dir) = &asset_dir {
            manifest = manifest.with_allowed_path(
                format!("ro:{}", asset_dir.to_string_lossy()),
                READONLY_ASSET_MOUNT,
            );
        }
        let plugin =
            extism::Plugin::new(WasmInput::Manifest(manifest), common_imports(), with_wasi)
                .map_err(|e| anyhow!(e).context("加载 extism 插件失败"))?;
        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }
        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
            tmp,
            command,
        })
    }
}

impl RenProcessor for ExtismProcessor {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput> {
        let streams: AssetStreams = Arc::new(Mutex::new(HashMap::new()));
        let ctx = PluginContext::new(
            None,
            self.tmp.path().to_path_buf(),
            streams,
            self.command.clone(),
        );
        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<ProcessorOutput> = plugin
            .call_with_host_context(&self.function, Json(doc), ctx)
            .map_err(|e| anyhow!(e).context("调用 extism 插件失败"))?;
        Ok(output.0)
    }
}
