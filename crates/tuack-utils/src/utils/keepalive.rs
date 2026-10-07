//! 产物返回时的临时目录保活包装。

use std::io::Read;
use std::sync::Arc;

use tempfile::TempDir;

/// 包装一个文件流并持有临时目录，保证其存活期不短于本流。
pub struct KeepAliveReader {
    inner: std::fs::File,
    _keepalive: Arc<TempDir>,
}

impl KeepAliveReader {
    pub fn new(inner: std::fs::File, keepalive: Arc<TempDir>) -> Self {
        Self {
            inner,
            _keepalive: keepalive,
        }
    }
}

impl Read for KeepAliveReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
