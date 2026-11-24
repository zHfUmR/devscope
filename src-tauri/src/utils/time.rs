//! 时间格式化小工具。
//!
//! 诊断报告需要一个可读、可跨时区解释的生成时间。这里不引入日期库，直接用
//! 标准库的 `SystemTime` 和公历换算实现 RFC 3339（UTC）。换算使用 Howard
//! Hinnant 的 `civil_from_days` 算法，只依赖整数运算。

use std::time::{SystemTime, UNIX_EPOCH};

/// 把 `SystemTime` 转成 Unix 毫秒。早于 1970 年的时间按 0 处理。
pub fn epoch_millis(time: SystemTime) -> u64 {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    duration.as_secs() * 1000 + u64::from(duration.subsec_millis())
}

/// 把 `SystemTime` 格式化成 RFC 3339 UTC 字符串，保留毫秒。
///
/// 例如 `2026-11-24T16:03:21.042Z`。早于 1970 年的时间会被钳制到纪元起点。
pub fn format_rfc3339_utc(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let epoch_seconds = duration.as_secs();
    let millis = duration.subsec_millis();

    let days = i64::try_from(epoch_seconds / 86_400).unwrap_or(i64::MAX);
    let seconds_of_day = epoch_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);

    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// 把「1970-01-01 之后的天数」换算成公历年月日。
///
/// 返回值中的年份可能为负值（1970 年之前），当前调用方不会用到。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // 把纪元平移到 0000-03-01，使闰年落在年末，简化月份计算。
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted / 146_097
    } else {
        (shifted - 146_096) / 146_097
    };
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let year = year_of_era + era * 400;

    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_index = (5 * day_of_year + 2) / 153; // [0, 11]，从 3 月开始
    let day = day_of_year - (153 * month_index + 2) / 5 + 1; // [1, 31]
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn formats_unix_epoch() {
        assert_eq!(format_rfc3339_utc(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        assert_eq!(epoch_millis(UNIX_EPOCH), 0);
    }

    #[test]
    fn formats_known_instants() {
        assert_eq!(format_rfc3339_utc(at(1_700_000_000)), "2023-11-14T22:13:20.000Z");
        assert_eq!(format_rfc3339_utc(at(1_709_164_800)), "2024-02-29T00:00:00.000Z");
        assert_eq!(format_rfc3339_utc(at(1_767_225_600)), "2026-01-01T00:00:00.000Z");
    }

    #[test]
    fn keeps_milliseconds() {
        let time = at(1_700_000_000) + Duration::from_millis(42);

        assert_eq!(format_rfc3339_utc(time), "2023-11-14T22:13:20.042Z");
        assert_eq!(epoch_millis(time), 1_700_000_000_042);
    }
}
