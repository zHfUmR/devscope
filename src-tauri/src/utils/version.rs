//! 版本字符串比较。
//!
//! Android platform / build-tools、iOS runtime 都是「带数字的字符串」，
//! 直接按字典序会得到 `android-9` > `android-36` 这种错误结果。这里只比较
//! 数字段，字符串本身只作为最后的稳定排序条件。

use std::cmp::Ordering;

/// 按数字段升序比较两个版本字符串。
///
/// 例如 `android-37.0` > `android-36`，`36.0.0` > `35.0.2`。
/// 没有数字的字符串会退化为普通字典序。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let left = version_parts(a);
    let right = version_parts(b);

    for (left, right) in left.iter().zip(right.iter()) {
        match left.cmp(right) {
            Ordering::Equal => {}
            other => return other,
        }
    }

    left.len().cmp(&right.len()).then_with(|| a.cmp(b))
}

/// 提取字符串里的连续数字段，忽略分隔符和前缀。
fn version_parts(text: &str) -> Vec<u64> {
    text.split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numeric_segments_not_text() {
        assert_eq!(compare_versions("android-37.0", "android-36"), Ordering::Greater);
        assert_eq!(compare_versions("android-9", "android-36"), Ordering::Less);
        assert_eq!(compare_versions("36.0.0", "36.0"), Ordering::Greater);
    }

    #[test]
    fn falls_back_to_text_for_equal_versions() {
        assert_eq!(compare_versions("android-36", "platform-36"), Ordering::Less);
    }
}
