//! 源码位置与 span 包装。

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 源码中的字节区间 `[start, stop)`
///
/// 叶子 inline 中 Code 的区间为空（`start == stop`，指向开头反引号），其余叶子精确覆盖其
/// 源码文本；块节点仅作定位，`start` 可靠。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Span {
    pub start: usize,
    pub stop: usize,
}

impl Span {
    pub fn new(start: usize, stop: usize) -> Self {
        Self { start, stop }
    }

    /// 从源码切片。
    ///
    /// # Panics
    ///
    /// 当 `start`/`stop` 越界、落在非 UTF-8 字符边界，或 `start > stop` 时 panic。
    pub fn str<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.stop]
    }
}

impl From<(usize, usize)> for Span {
    fn from((start, stop): (usize, usize)) -> Self {
        Self { start, stop }
    }
}

/// 值 + 可选的源码位置
///
/// 叶子 inline（Text/Code/Latex/Html/FootnoteReference）与块（`BlockKind::Empty` 除外）为
/// `Some`；SoftBreak/LineBreak/Link/LinkReference/Image/Autolink 与复合 inline
/// （Emphasis/Strong/Strikethrough）为 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Spanned<T> {
    pub value: T,
    pub span: Option<Span>,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Option<Span>) -> Self {
        Self { value, span }
    }

    /// 构造带必现 span 的值（方法名避开与类型同名，见 clippy `same_name_method`）。
    pub fn with_span(value: T, span: Span) -> Self {
        Self {
            value,
            span: Some(span),
        }
    }

    pub fn plain(value: T) -> Self {
        Self { value, span: None }
    }

    /// 就地转换值，保留 span。
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }

    pub fn as_ref(&self) -> Spanned<&T> {
        Spanned {
            value: &self.value,
            span: self.span,
        }
    }
}
