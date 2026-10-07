//! 块级节点。

use super::inline::Inline;
use super::list::List;
use super::table::Table;
use crate::span::Spanned;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 块级构造
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "data"))]
pub enum BlockKind {
    Paragraph(Vec<Inline>),
    /// ATX（`# Heading`）或 Setext（`===`）标题
    Heading(Heading),
    ThematicBreak,
    BlockQuote(Vec<Block>),
    List(List),
    CodeBlock(CodeBlock),
    HtmlBlock(String),
    Definition(LinkDefinition),
    Table(Table),
    FootnoteDefinition(FootnoteDefinition),
    /// 容器块（`:::kind` 或 `:::{.kind}`）
    Container(Container),
    LatexBlock(String),
    Empty,
}

/// 块节点别名（[`BlockKind`] + 可选源码 span）
pub type Block = Spanned<BlockKind>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Container {
    pub kind: String,
    pub params: Vec<ContainerParam>,
    pub blocks: Vec<Block>,
}

/// 来源可为键值对（`:::{caption="标题"}`）或无值的裸属性（`:::{right}`）；
/// 裸属性按布尔标记处理，后续可扩展混合列表（`:::{aa, bb, b=c, c=d}`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "data"))]
pub enum ContainerParam {
    /// 键值对，如 `caption="标题"`、`id="x"`
    KeyValue(String, String),
    /// 无值单属性（布尔标记），如 `right`
    Flag(String),
}

impl ContainerParam {
    /// 返回参数名（键）。
    pub fn key(&self) -> &str {
        match self {
            ContainerParam::KeyValue(k, _) => k,
            ContainerParam::Flag(k) => k,
        }
    }

    /// 返回参数值：键值对返回 `Some(值)`，裸属性返回 `None`。
    pub fn value(&self) -> Option<&str> {
        match self {
            ContainerParam::KeyValue(_, v) => Some(v),
            ContainerParam::Flag(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Heading {
    pub kind: HeadingKind,
    pub content: Vec<Inline>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "data"))]
pub enum HeadingKind {
    /// ATX 标题（`# Heading`），`u8` 为级别 1-6
    Atx(u8),
    /// Setext 标题
    Setext(SetextHeading),
}

/// Setext 标题种类
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SetextHeading {
    /// `===` 下划线
    Level1,
    /// `---` 下划线
    Level2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CodeBlock {
    pub kind: CodeBlockKind,
    pub literal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "data"))]
pub enum CodeBlockKind {
    /// 缩进代码块（4 空格缩进）
    Indented,
    /// 围栏代码块，`info` 为围栏后的信息串（语言标识）
    Fenced { info: Option<String> },
}

/// `label` 是纯文本标识符（可含 `*` 等字符但不解析行内语法），用于匹配引用链接。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LinkDefinition {
    pub label: String,
    pub destination: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FootnoteDefinition {
    pub label: String,
    pub blocks: Vec<Block>,
}
