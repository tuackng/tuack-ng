use crate::prelude::*;
use debug_tree::{TreeBuilder, TreeConfig, TreeSymbols};
use owo_colors::OwoColorize;

/// 加载消息级别
#[derive(Clone, Debug, Copy)]
pub enum LoadMessageLevel {
    Warn,
    Error,
    Note,
}

/// 单条加载消息
#[derive(Clone, Debug)]
pub struct LoadMessage {
    pub level: LoadMessageLevel,
    pub message: String,
}

impl LoadMessage {
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            level: LoadMessageLevel::Warn,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: LoadMessageLevel::Error,
            message: message.into(),
        }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self {
            level: LoadMessageLevel::Note,
            message: message.into(),
        }
    }
}

/// 某个配置层级的加载消息集合
#[derive(Clone, Debug, Default)]
pub struct LoadMessages {
    /// 该层级在加载信息树中显示的分支标题，形如 `[contest] xxx`
    ///
    /// 仅用于展示（见 [`LoadMessages::render_tree`]），不参与层级查找：由调用方拼接，
    /// `[...]` 为层级类型（`contest`/`day`/`problem`），`xxx` 为该层的配置名；根层级留空。
    pub name: String,
    /// 该层级产生的消息
    pub messages: Vec<LoadMessage>,
    /// 子层级的消息集合
    pub sub: Vec<LoadMessages>,
}

impl LoadMessages {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            messages: Vec::new(),
            sub: Vec::new(),
        }
    }

    pub fn push(&mut self, message: LoadMessage) {
        self.messages.push(message);
    }

    pub fn count(&self) -> usize {
        self.messages.len() + self.sub.iter().map(|v| v.count()).sum::<usize>()
    }

    pub fn count_errors(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| matches!(m.level, LoadMessageLevel::Error))
            .count()
            + self.sub.iter().map(|v| v.count_errors()).sum::<usize>()
    }

    pub fn count_warnings(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| matches!(m.level, LoadMessageLevel::Warn))
            .count()
            + self.sub.iter().map(|v| v.count_warnings()).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// 渲染消息树；`use_color` 表示是否展示颜色，
    /// 具体决定状态符号用彩色 `*` 还是纯文本 `[W]`/`[E]`/`[N]`。
    pub fn render_tree(&self, tree: &mut TreeBuilder, use_color: bool) {
        for leaf in &self.messages {
            let level_string = if use_color {
                match leaf.level {
                    LoadMessageLevel::Warn => "*".yellow().bold().to_string(),
                    LoadMessageLevel::Error => "*".red().bold().to_string(),
                    LoadMessageLevel::Note => "*".blue().bold().to_string(),
                }
            } else {
                match leaf.level {
                    LoadMessageLevel::Warn => "[W]".to_string(),
                    LoadMessageLevel::Error => "[E]".to_string(),
                    LoadMessageLevel::Note => "[N]".to_string(),
                }
            };

            tree.add_leaf(&format!("{level_string} {}", leaf.message));
        }

        for branch in &self.sub {
            if branch.is_empty() {
                continue;
            }
            let _branch = tree.add_branch(&branch.name);
            branch.render_tree(tree, use_color);
        }
    }

    /// 渲染错误消息树；`use_color` 表示是否展示颜色，
    /// 具体决定错误符号用彩色 `*` 还是纯文本 `[E]`。
    pub fn render_errors(&self, tree: &mut TreeBuilder, use_color: bool) {
        for leaf in &self.messages {
            if !matches!(leaf.level, LoadMessageLevel::Error) {
                continue;
            }
            let level_string = if use_color {
                "*".red().bold().to_string()
            } else {
                "[E]".to_string()
            };

            tree.add_leaf(&format!("{level_string} {}", leaf.message));
        }

        for branch in &self.sub {
            if branch.count_errors() == 0 {
                continue;
            }
            let _branch = tree.add_branch(&branch.name);
            branch.render_errors(tree, use_color);
        }
    }
}

/// 配置加载上下文，记录层级消息与迁移状态
#[derive(Clone, Debug, Default)]
pub struct LoadContext {
    /// 根层级的加载消息
    pub root: LoadMessages,
    current_path: Vec<usize>,
    /// 本次加载是否发生过版本迁移
    pub migrated: bool,
    force_migrate: bool,
    /// 迁移过程中产生的提示信息，键为原版本号
    pub migrated_notices: IndexMap<i32, &'static str>,
}

impl LoadContext {
    fn current(&mut self) -> &mut LoadMessages {
        let mut node = &mut self.root;
        for &idx in &self.current_path {
            node = &mut node.sub[idx];
        }
        node
    }

    pub fn new() -> Self {
        Self {
            root: LoadMessages::new(),
            current_path: Vec::new(),
            migrated: false,
            force_migrate: false,
            migrated_notices: IndexMap::new(),
        }
    }

    /// 创建允许执行强制迁移的加载上下文
    ///
    /// 迁移器的 [`MigraterMetadata::force`] 为 `true` 时，迁移涉及 `conf.json` 之外的改动，
    /// 配置加载默认拒绝自动执行；只有通过本构造器创建的上下文才授权执行这类迁移。
    ///
    /// [`MigraterMetadata::force`]: crate::config::migrate::base::MigraterMetadata::force
    pub fn new_force_migrate() -> Self {
        Self {
            force_migrate: true,
            ..Self::new()
        }
    }

    /// 是否已授权执行强制迁移
    pub fn force_migrate(&self) -> bool {
        self.force_migrate
    }

    /// 进入新的子层级，之后记录的消息归属该层级
    pub fn enter(&mut self) {
        let idx = {
            let node = self.current();
            node.sub.push(LoadMessages::new());
            node.sub.len() - 1
        };
        self.current_path.push(idx);
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.current().name = name.into();
    }

    /// 回到上一层，恢复进入前的归属层级
    pub fn ret(&mut self) {
        self.current_path.pop();
    }

    pub fn emit_note(&mut self, message: impl Into<String>) {
        self.current().push(LoadMessage::note(message));
    }

    pub fn emit_warn(&mut self, message: impl Into<String>) {
        self.current().push(LoadMessage::warn(message));
    }

    pub fn emit_error(&mut self, message: impl Into<String>) {
        self.current().push(LoadMessage::error(message));
    }

    /// 渲染消息树；`use_color` 表示是否展示颜色，为 `false` 时移除 ANSI。
    pub fn render_tree(&self, use_color: bool) -> String {
        let mut tree = TreeBuilder::new();
        tree.set_config_override(
            TreeConfig::new()
                .symbols(TreeSymbols::new().leaf("─ "))
                .indent(2),
        );
        self.root.render_tree(&mut tree, use_color);
        let text = tree.string();
        if use_color {
            text
        } else {
            anstream::adapter::strip_str(&text).to_string()
        }
    }

    /// 渲染错误消息树；`use_color` 表示是否展示颜色，为 `false` 时移除 ANSI。
    pub fn render_errors_tree(&self, use_color: bool) -> String {
        let mut tree = TreeBuilder::new();
        tree.set_config_override(
            TreeConfig::new()
                .symbols(TreeSymbols::new().leaf("─ "))
                .indent(2),
        );
        self.root.render_errors(&mut tree, use_color);
        let text = tree.string();
        if use_color {
            text
        } else {
            anstream::adapter::strip_str(&text).to_string()
        }
    }
}
