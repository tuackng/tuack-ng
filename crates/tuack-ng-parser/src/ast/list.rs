//! 列表节点。

use super::block::Block;
use crate::span::Spanned;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 列表
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct List {
    pub kind: ListKind,
    pub items: Vec<ListItem>,
}

/// 列表项节点别名（[`ListItemKind`] + 可选源码 span）
pub type ListItem = Spanned<ListItemKind>;

/// 列表种类
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "data"))]
pub enum ListKind {
    /// 有序列表（`1.`、`2.` …）；起始编号不保留
    Ordered,
    /// 无序列表（`-`、`*`、`+`）
    Bullet(ListBulletKind),
}

/// 无序列表的项目符号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ListBulletKind {
    Dash,
    Star,
    Plus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ListItemKind {
    pub blocks: Vec<Block>,
}

impl ListItemKind {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }
}
