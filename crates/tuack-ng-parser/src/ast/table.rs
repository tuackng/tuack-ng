//! 表格相关节点。

use super::inline::Inline;
use crate::span::Spanned;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 表格：行集合 + 列对齐
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// 每行是一组单元格；首行为表头（row 0）。
    pub rows: Vec<Vec<TableCell>>,
    /// 列对齐；`alignments.len() == 列数`。
    pub alignments: Vec<Alignment>,
}

/// 表格单元格节点别名（[`TableCellKind`] + 可选源码 span）
pub type TableCell = Spanned<TableCellKind>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableCellKind {
    pub content: Vec<Inline>,
    /// 横向合并的列数（`None` 视作 1）
    pub colspan: Option<usize>,
    /// 纵向合并的行数（`None` 视作 1）
    pub rowspan: Option<usize>,
    /// 是否为扩展表格语法（`<` / `^`）合并掉的标记单元格
    pub removed_by_extended_table: bool,
}

impl TableCellKind {
    pub fn new(content: Vec<Inline>) -> Self {
        Self {
            content,
            colspan: None,
            rowspan: None,
            removed_by_extended_table: false,
        }
    }
}

/// 单元格对齐
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Alignment {
    /// 未指定对齐
    #[default]
    None,
    Left,
    Center,
    Right,
}
