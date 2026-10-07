//! Markdown AST 节点定义。
//!
//! 每个节点通过 [`crate::span::Spanned`] 携带可选的源码位置。

pub mod block;
pub mod inline;
pub mod list;
pub mod table;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use block::{
    Block, BlockKind, CodeBlock, CodeBlockKind, Container, ContainerParam, FootnoteDefinition,
    Heading, HeadingKind, LinkDefinition, SetextHeading,
};
pub use inline::{
    Autolink, Image, ImageAttributes, Inline, InlineKind, Link, LinkKind, LinkReference,
    LinkReferenceKind,
};
pub use list::{List, ListBulletKind, ListItem, ListItemKind, ListKind};
pub use table::{Alignment, Table, TableCell, TableCellKind};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
