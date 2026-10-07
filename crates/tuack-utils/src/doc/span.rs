//! span（字节区间）与行的换算工具。

/// 计算 1 起行号对应的整行字节区间 `[start, end)`（不含换行符）。
/// 行号为 0 或越界时返回 None（行按换行符切分，末尾换行符之后仍计一行）。
pub fn line_to_byte_span(source: &str, line: usize) -> Option<tuack_ng_parser::Span> {
    if line == 0 {
        return None;
    }
    let mut cur = 1;
    let mut start = 0usize;
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            if cur == line {
                return Some(tuack_ng_parser::Span::new(start, i));
            }
            cur += 1;
            start = i + 1;
        }
    }
    if cur == line {
        Some(tuack_ng_parser::Span::new(start, source.len()))
    } else {
        None
    }
}

/// 将 span 起点拓展到其所在行的行尾（不含换行符）。
pub fn extend_to_line_end(
    source: &str,
    span: Option<tuack_ng_parser::Span>,
) -> Option<tuack_ng_parser::Span> {
    let s = span?;
    let start = s.start.min(source.len());
    let stop = source[start..]
        .find('\n')
        .map(|i| start + i)
        .unwrap_or(source.len());
    Some(tuack_ng_parser::Span::new(start, stop))
}
