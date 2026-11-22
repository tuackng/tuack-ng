/// 返回以 10 为底的对数的整数部分；`0` 得 `-inf`，负数得 `NaN`，正无穷与 `NaN` 原样返回
pub fn int_lg(num: f64) -> f64 {
    if num == 0.0 {
        return f64::NEG_INFINITY;
    }
    if num < 0.0 {
        return f64::NAN;
    }
    if !num.is_finite() {
        return num;
    }

    let mut n = 0;
    let mut temp = num;

    if temp >= 10.0 {
        while temp >= 10.0 {
            temp /= 10.0;
            n += 1;
        }
    } else if temp < 1.0 {
        while temp < 1.0 {
            temp *= 10.0;
            n -= 1;
        }
    }

    n as f64
}

/// 用 `{,}` 作为千分位分隔符格式化数字：只分组整数部分，小数部分原样保留；非有限值原样返回
pub fn comma(num: f64) -> String {
    grouped(num, "{,}")
}

/// 用 `\,` 作为千分位分隔符格式化数字：只分组整数部分，小数部分原样保留；非有限值原样返回
pub fn spaces(num: f64) -> String {
    grouped(num, "\\,")
}

/// 格式化数字：`style` 为 `"x"` 用 LaTeX 科学记数法，`","` 用 `{,}` 千分位分隔，
/// `" "` 用 `\,` 分隔；`None` 或未知样式时，10^3 的倍数用科学记数法，其余用 `{,}` 千分位。
///
/// 带小数的数字只有 `"x"` 会转成科学记数，其余样式只分组整数部分；`inf`、`NaN` 原样返回。
pub fn hn(num: f64, style: Option<&str>) -> String {
    if num == 0.0 {
        return "0".to_string();
    }

    match style {
        Some("x") => sci(num),
        Some(" ") => spaces(num),
        Some(",") => comma(num),
        _ if trailing_zeros(num.abs()).is_some_and(|zeros| zeros >= SCI_MIN_ZEROS) => sci(num),
        _ => comma(num),
    }
}

/// 把数字格式化为 LaTeX 科学记数法：系数为 1 时写成 `10^{k}`，其余写成 `c \times 10^{k}`
///
/// `0` 返回 `"0"`；`inf`、`-inf`、`NaN` 原样返回。
pub fn sci(num: f64) -> String {
    if num == 0.0 {
        return "0".to_string();
    }

    let (neg, abs) = if num < 0.0 { ("-", -num) } else { ("", num) };

    if !abs.is_finite() {
        return format!("{neg}{abs}");
    }

    let text = format!("{abs:e}");
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("有限 f64 的 `{:e}` 输出形如 `系数 e 指数`");
    if mantissa == "1" {
        format!("{neg}10^{{{exponent}}}")
    } else {
        format!("{neg}{mantissa} \\times 10^{{{exponent}}}")
    }
}

/// 把升序整数列表压缩为区间表示，返回 `$` 包裹的 LaTeX 公式
///
/// 连续区间长度大于 2 时写作 `a \sim b`；空列表返回 `$$`
pub fn cases(items: &[i32]) -> String {
    if items.is_empty() {
        return "$$".to_string();
    }

    let mut result = Vec::new();
    let mut start = items[0];
    let mut end = items[0];

    for &num in &items[1..] {
        if num == end + 1 {
            end = num;
        } else {
            if start == end {
                result.push(format!("{}", start));
            } else if start + 1 == end {
                result.push(format!("{}", start));
                result.push(format!("{}", end));
            } else {
                result.push(format!("{} \\sim {}", start, end));
            }
            start = num;
            end = num;
        }
    }

    if start == end {
        result.push(format!("{}", start));
    } else if start + 1 == end {
        result.push(format!("{}", start));
        result.push(format!("{}", end));
    } else {
        result.push(format!("{} \\sim {}", start, end));
    }

    format!("${}$", result.join(","))
}

/// 自动模式下改用科学记数法所需的末尾零个数
const SCI_MIN_ZEROS: usize = 3;

/// 给十进制数字串每三位插入 `sep`
fn group_digits(digits: &str, sep: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 * sep.len());
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(ch);
    }
    out
}

/// 给数字的整数部分加千分位分隔符，小数部分原样保留
fn grouped(num: f64, sep: &str) -> String {
    let (neg, abs) = if num < 0.0 { ("-", -num) } else { ("", num) };

    if !abs.is_finite() {
        return format!("{neg}{abs}");
    }

    let text = format!("{abs}");
    let body = match text.split_once('.') {
        Some((integer, fraction)) => format!("{}.{}", group_digits(integer, sep), fraction),
        None => group_digits(&text, sep),
    };
    format!("{neg}{body}")
}

/// 整数末尾零的个数；带小数或非有限值时返回 `None`
fn trailing_zeros(abs: f64) -> Option<usize> {
    if abs.fract() != 0.0 {
        return None;
    }
    let text = format!("{abs}");
    Some(text.len() - text.trim_end_matches('0').len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(comma(1234567.0), "1{,}234{,}567");
        assert_eq!(comma(-1234.0), "-1{,}234");
        assert_eq!(comma(999.0), "999");
        assert_eq!(comma(1234.5), "1{,}234.5");
        assert_eq!(comma(-1234.5), "-1{,}234.5");
        assert_eq!(comma(1e15), "1{,}000{,}000{,}000{,}000{,}000");
        assert_eq!(comma(f64::INFINITY), "inf");
        assert_eq!(comma(f64::NAN), "NaN");

        assert_eq!(spaces(1234567.0), "1\\,234\\,567");
        assert_eq!(spaces(-1234.0), "-1\\,234");
        assert_eq!(spaces(999.0), "999");
        assert_eq!(spaces(1234.5), "1\\,234.5");
    }

    #[test]
    fn auto_uses_sci_for_multiples_of_1000() {
        assert_eq!(hn(1000.0, None), "10^{3}");
        assert_eq!(hn(2000.0, None), "2 \\times 10^{3}");
        assert_eq!(hn(1230000.0, None), "1.23 \\times 10^{6}");
        assert_eq!(hn(1000000.0, None), "10^{6}");
        assert_eq!(hn(12300.0, None), "12{,}300");
        assert_eq!(hn(1234567.0, None), "1{,}234{,}567");
        assert_eq!(hn(999.0, None), "999");
    }

    #[test]
    fn sci_formats_any_magnitude() {
        assert_eq!(sci(1000.0), "10^{3}");
        assert_eq!(sci(1234.0), "1.234 \\times 10^{3}");
        assert_eq!(sci(-1234.0), "-1.234 \\times 10^{3}");
        assert_eq!(sci(0.001), "10^{-3}");
        assert_eq!(sci(0.0), "0");
        assert_eq!(sci(1e300), "10^{300}");
        assert_eq!(sci(f64::INFINITY), "inf");
        assert_eq!(sci(f64::NAN), "NaN");
    }

    #[test]
    fn explicit_styles() {
        assert_eq!(hn(1234567.0, Some(",")), "1{,}234{,}567");
        assert_eq!(hn(1234567.0, Some(" ")), "1\\,234\\,567");
        assert_eq!(hn(1234567.0, Some("x")), "1.234567 \\times 10^{6}");
        assert_eq!(hn(1234567.0, Some("foo")), "1{,}234{,}567");
    }

    #[test]
    fn decimals_group_the_integer_part() {
        assert_eq!(hn(1234.5, None), "1{,}234.5");
        assert_eq!(hn(1234.5, Some(",")), "1{,}234.5");
        assert_eq!(hn(1234.5, Some(" ")), "1\\,234.5");
        assert_eq!(hn(12.5, None), "12.5");
        assert_eq!(hn(1234.5, Some("x")), "1.2345 \\times 10^{3}");
        assert_eq!(hn(0.3, Some("x")), "3 \\times 10^{-1}");
        assert_eq!(hn(0.001, Some("x")), "10^{-3}");
    }

    #[test]
    fn extremes_do_not_panic_or_hang() {
        assert_eq!(hn(1e19, None), "10^{19}");
        assert_eq!(hn(1e300, Some("x")), "10^{300}");
        assert_eq!(hn(f64::INFINITY, None), "inf");
        assert_eq!(hn(f64::NEG_INFINITY, None), "-inf");
        assert_eq!(hn(f64::NAN, None), "NaN");
        assert_eq!(hn(-0.0, None), "0");
    }
}
