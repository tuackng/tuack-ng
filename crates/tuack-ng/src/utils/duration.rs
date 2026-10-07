use std::time::Duration;

/// 将时长格式化为可读字符串：小于 1s 用毫秒（两位小数），否则用秒（三位小数）。
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();

    if secs < 1.0 {
        let millis = duration.as_millis();
        format!("{:.2}ms", millis as f64)
    } else {
        format!("{:.3}s", secs)
    }
}
