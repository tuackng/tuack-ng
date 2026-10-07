//! Typst 渲染器。
//!
//! 主要输出形式：
//! - 段落包裹 `#par[...]`，文本包裹 `#"..."`。
//! - 表格 `#figure(table(columns: (...), align: (...), cells))`
//! - 合并单元格用 `table.cell(colspan:, rowspan:)` 输出；`removed_by_extended_table` 的标记单元格跳过

use crate::ast::block::{BlockKind, CodeBlockKind, ContainerParam, HeadingKind, SetextHeading};
use crate::ast::inline::InlineKind;
use crate::ast::list::ListKind;
use crate::ast::{Block, Document, Inline};
use std::collections::HashMap;

/// 渲染为 Typst 字符串
pub fn render_typst(doc: &Document) -> String {
    let footnotes = collect_footnotes(doc);
    let mut out = String::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        render_block(&block.value, &footnotes, &mut out);
    }
    out.push('\n');
    out
}

/// 收集脚注定义，label -> blocks：渲染引用时把定义内容内联到引用点。
fn collect_footnotes(doc: &Document) -> HashMap<String, Vec<Block>> {
    let mut map = HashMap::new();
    for block in &doc.blocks {
        if let BlockKind::FootnoteDefinition(fd) = &block.value {
            map.entry(fd.label.clone())
                .or_insert_with(|| fd.blocks.clone());
        }
    }
    map
}

fn render_block(block: &BlockKind, footnotes: &HashMap<String, Vec<Block>>, out: &mut String) {
    match block {
        BlockKind::Paragraph(inlines) => {
            out.push_str("#par[");
            render_inlines(inlines, footnotes, out);
            out.push(']');
        }
        BlockKind::Heading(h) => {
            let level = match h.kind {
                HeadingKind::Atx(level) => level,
                HeadingKind::Setext(SetextHeading::Level1) => 1,
                HeadingKind::Setext(SetextHeading::Level2) => 2,
            };
            out.push_str(&format!("#heading(level: {level}, ["));
            render_inlines(&h.content, footnotes, out);
            out.push_str("])");
        }
        BlockKind::ThematicBreak => out.push_str("#thematic-break"),
        BlockKind::BlockQuote(blocks) => {
            out.push_str("#quote(block: true)[");
            for (i, b) in blocks.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                render_block(&b.value, footnotes, out);
            }
            out.push(']');
        }
        BlockKind::List(list) => {
            let open = match &list.kind {
                ListKind::Ordered => "#enum(",
                ListKind::Bullet(_) => "#list(",
            };
            out.push_str(open);
            for item in &list.items {
                out.push_str("\n  [");
                for (j, b) in item.value.blocks.iter().enumerate() {
                    if j > 0 {
                        out.push(' ');
                    }
                    // 列表项内段落不包裹 `#par`，直接渲染 inlines。
                    if let BlockKind::Paragraph(inlines) = &b.value {
                        render_inlines(inlines, footnotes, out);
                    } else {
                        render_block(&b.value, footnotes, out);
                    }
                }
                out.push_str("],");
            }
            out.push_str("\n)");
        }
        BlockKind::CodeBlock(cb) => {
            let lang = match &cb.kind {
                CodeBlockKind::Fenced { info } => info.clone().unwrap_or_default(),
                CodeBlockKind::Indented => String::new(),
            };
            let literal = cb.literal.trim_end_matches(['\n', '\r']);
            out.push_str("#raw(block: true");
            if !lang.is_empty() {
                out.push_str(&format!(", lang: \"{lang}\""));
            }
            out.push_str(&format!(", \"{}\")", escape_typst(literal)));
        }
        BlockKind::HtmlBlock(html) => {
            out.push_str(&format!("#raw[{}]", escape_typst(html)));
        }
        BlockKind::Definition(_) => {}
        BlockKind::Table(table) => render_table(table, footnotes, out),
        BlockKind::FootnoteDefinition(_) => {}
        BlockKind::Container(c) => {
            // figure 输出 `#figure(caption:)[..]`，其他 kind 解包渲染内容。
            if c.kind == "figure" {
                out.push_str("#figure");
                let mut args = Vec::new();
                if let Some(ContainerParam::KeyValue(_, caption)) =
                    c.params.iter().find(|p| p.key() == "caption")
                {
                    args.push(format!("caption: [{}]", escape_typst(caption)));
                }
                if !args.is_empty() {
                    out.push_str(&format!("({})", args.join(", ")));
                }
                out.push('[');
                for (i, b) in c.blocks.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    render_block(&b.value, footnotes, out);
                }
                out.push(']');
            } else if c.kind == "align" {
                // `:::align{right}` / `:::align{center}` / `:::align{left}` 对齐容器（裸参数）。
                let align = c.params.iter().find_map(|p| match p {
                    ContainerParam::Flag(k)
                        if matches!(k.as_str(), "left" | "center" | "right") =>
                    {
                        Some(k.clone())
                    }
                    _ => None,
                });
                match align {
                    Some(align) => {
                        out.push_str(&format!("#align({align})["));
                        for (i, b) in c.blocks.iter().enumerate() {
                            if i > 0 {
                                out.push('\n');
                            }
                            render_block(&b.value, footnotes, out);
                        }
                        out.push(']');
                    }
                    // 无对齐参数：解包渲染内容。
                    None => {
                        for (i, b) in c.blocks.iter().enumerate() {
                            if i > 0 {
                                out.push('\n');
                            }
                            render_block(&b.value, footnotes, out);
                        }
                    }
                }
            } else {
                for (i, b) in c.blocks.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    render_block(&b.value, footnotes, out);
                }
            }
        }
        BlockKind::LatexBlock(latex) => {
            out.push_str(&format!("#mi(block: true, \"{}\")", escape_typst(latex)));
        }
        BlockKind::Empty => {}
    }
}

fn render_inlines(inlines: &[Inline], footnotes: &HashMap<String, Vec<Block>>, out: &mut String) {
    // 归并相邻 Text，使连续文本进入 `#"..."` 内部。
    let mut pending = String::new();

    fn flush(pending: &mut String, out: &mut String) {
        if !pending.is_empty() {
            out.push_str(&format!("#\"{}\"", escape_typst(pending)));
            pending.clear();
        }
    }

    for inline in inlines {
        match &inline.value {
            InlineKind::Text(t) => pending.push_str(t),
            // Markdown 软换行 -> typst `#linebreak()`（保留换行）。
            InlineKind::SoftBreak => {
                flush(&mut pending, out);
                out.push_str("#linebreak()");
            }
            other => {
                flush(&mut pending, out);
                render_inline(other, footnotes, out);
            }
        }
    }
    flush(&mut pending, out);
}

fn render_inline(inline: &InlineKind, footnotes: &HashMap<String, Vec<Block>>, out: &mut String) {
    match inline {
        InlineKind::Text(_) => unreachable!("Text 由 render_inlines 归并处理"),
        InlineKind::SoftBreak => unreachable!("SoftBreak 由 render_inlines 归并处理"),
        InlineKind::LineBreak => {
            // typst 硬换行：反斜杠续行。否则 `\n` 会被折叠成空格。
            out.push('\\');
            out.push('\n');
        }
        InlineKind::Code(code) => out.push_str(&format!("#raw(\"{}\")", escape_typst(code))),
        InlineKind::Latex(latex) => {
            out.push_str(&format!("#mi(block: false, \"{}\")", escape_typst(latex)))
        }
        InlineKind::Html(html) => out.push_str(&format!("#raw[{}]", escape_typst(html))),
        InlineKind::Link(link) => {
            let mut args = vec![format!("\"{}\"", escape_typst(&link.destination))];
            if let Some(title) = &link.title {
                args.push(format!("title: \"{}\"", escape_typst(title)));
            }
            out.push_str(&format!("#link({})[", args.join(", ")));
            render_inlines(&link.children, footnotes, out);
            out.push(']');
        }
        InlineKind::LinkReference(r) => {
            out.push_str(&format!("#link(\"{}\")[", escape_typst(&r.destination)));
            render_inlines(&r.text, footnotes, out);
            out.push(']');
        }
        InlineKind::Autolink(a) => {
            out.push_str(&format!("#link(\"{}\")", escape_typst(&a.url)));
        }
        InlineKind::Image(img) => {
            out.push_str(&format!(
                "#box(image(\"{}\", alt: \"{}\"",
                escape_typst(&img.destination),
                escape_typst(&img.alt)
            ));
            if let Some(attr) = &img.attr {
                if let Some(width) = &attr.width {
                    out.push_str(&format!(", width: {width}"));
                }
                if let Some(height) = &attr.height {
                    out.push_str(&format!(", height: {height}"));
                }
            }
            out.push_str("))");
        }
        InlineKind::Emphasis(children) => {
            out.push_str("#emph[");
            render_inlines(children, footnotes, out);
            out.push(']');
        }
        InlineKind::Strong(children) => {
            out.push_str("#strong[");
            render_inlines(children, footnotes, out);
            out.push(']');
        }
        InlineKind::Strikethrough(children) => {
            out.push_str("#strike[");
            render_inlines(children, footnotes, out);
            out.push(']');
        }
        InlineKind::FootnoteReference(label) => {
            // 内联定义内容到引用点；找不到定义时输出 `[^label]`。
            if let Some(blocks) = footnotes.get(label) {
                out.push_str("#footnote[");
                for (i, b) in blocks.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    if let BlockKind::Paragraph(inlines) = &b.value {
                        render_inlines(inlines, footnotes, out);
                    } else {
                        render_block(&b.value, footnotes, out);
                    }
                }
                out.push(']');
            } else {
                out.push_str(&format!("[^{}]", escape_typst(label)));
            }
        }
        InlineKind::Empty => {}
    }
}

/// 渲染表格：输出 `#figure(table(...))` 格式。
fn render_table(
    table: &crate::ast::Table,
    footnotes: &HashMap<String, Vec<Block>>,
    out: &mut String,
) {
    if table.rows.is_empty() {
        return;
    }
    let columns = table
        .alignments
        .len()
        .max(table.rows.iter().map(|r| r.len()).max().unwrap_or(0));
    out.push_str("#figure(table(\n");
    out.push_str(&format!("  columns: ({columns}),\n"));

    // align: 无对齐时全部 `center + horizon`，有对齐时用实际值。
    if table.alignments.is_empty() {
        out.push_str("  align: (center + horizon),\n");
    } else {
        let aligns: Vec<&str> = table
            .alignments
            .iter()
            .map(|a| match a {
                crate::ast::Alignment::Left => "left + horizon",
                crate::ast::Alignment::Center => "center + horizon",
                crate::ast::Alignment::Right => "right + horizon",
                crate::ast::Alignment::None => "center + horizon",
            })
            .collect();
        out.push_str(&format!("  align: ({}),\n", aligns.join(", ")));
    }

    for row in &table.rows {
        let mut cells_line = String::new();
        let mut has_cell = false;
        for cell in row {
            if cell.value.removed_by_extended_table {
                continue;
            }
            let mut content = String::new();
            render_inlines(&cell.value.content, footnotes, &mut content);
            // 收集 >1 的 colspan/rowspan。
            let mut cell_parts = Vec::new();
            if let Some(colspan) = cell.value.colspan {
                if colspan > 1 {
                    cell_parts.push(format!("colspan: {colspan}"));
                }
            }
            if let Some(rowspan) = cell.value.rowspan {
                if rowspan > 1 {
                    cell_parts.push(format!("rowspan: {rowspan}"));
                }
            }
            let cell_repr = if cell_parts.is_empty() {
                format!("[{content}]")
            } else {
                format!("table.cell({})[{content}]", cell_parts.join(", "))
            };
            cells_line.push_str(&format!("  {cell_repr},"));
            has_cell = true;
        }
        if has_cell {
            out.push_str(&cells_line);
            out.push('\n');
        }
    }
    out.push_str("))");
}

fn escape_typst(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => r"\\".to_string(),
            '"' => "\\\"".to_string(),
            '\t' => r"\t".to_string(),
            '\n' => r"\n".to_string(),
            '\r' => r"\r".to_string(),
            _ => c.to_string(),
        })
        .collect()
}
