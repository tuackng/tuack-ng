//! 状态标签的文本与颜色。

/// 带颜色的状态标签
pub trait StatusLabel {
    fn label(&self) -> String;
}
