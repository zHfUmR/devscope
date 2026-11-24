//! 从 Android 的 `build.gradle` / `build.gradle.kts` 中提取关键配置。
//!
//! 这里不执行 Gradle，也不做完整的 Groovy / Kotlin 语法分析：Flutter 模板的
//! `android { defaultConfig { ... } }` 结构非常固定，逐行识别键值就够了。
//! 版本可能写成 `flutter.compileSdkVersion`，原样保留，由 UI 提示「委托给 Flutter」。

/// Gradle 文件中与构建相关的字段。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct GradleConfig {
    pub application_id: Option<String>,
    pub namespace: Option<String>,
    pub compile_sdk: Option<String>,
    pub min_sdk: Option<String>,
    pub target_sdk: Option<String>,
}

/// 解析 Gradle 文件。只取第一次出现且不为空的值，忽略 flavor 等后续覆盖。
pub(super) fn parse(content: &str) -> GradleConfig {
    let stripped = strip_comments(content);
    let mut config = GradleConfig::default();

    for raw in stripped.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        set_if_empty(&mut config.application_id, value_after_key(line, &["applicationId"]));
        set_if_empty(&mut config.namespace, value_after_key(line, &["namespace"]));
        set_if_empty(&mut config.compile_sdk, value_after_key(line, &["compileSdkVersion", "compileSdk"]));
        set_if_empty(&mut config.min_sdk, value_after_key(line, &["minSdkVersion", "minSdk"]));
        set_if_empty(&mut config.target_sdk, value_after_key(line, &["targetSdkVersion", "targetSdk"]));
    }

    config
}

fn set_if_empty(target: &mut Option<String>, value: Option<String>) {
    if target.is_none() {
        *target = value;
    }
}

/// 读取行首键后面的值，兼容 Groovy（`minSdk 24`）和 Kotlin（`minSdk = 24`）。
///
/// 键后面必须是空白、`=` 或行尾，避免把 `applicationIdSuffix` 当成 `applicationId`。
fn value_after_key(line: &str, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };

        if !rest.is_empty() && !rest.starts_with([' ', '\t', '=']) {
            continue;
        }

        let value = rest.trim_start().trim_start_matches('=').trim().trim_end_matches(';').trim();

        let value = unquote(value);
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn unquote(value: &str) -> &str {
    let value = value.trim();

    if let Some(inner) = value.strip_prefix('\'').and_then(|text| text.strip_suffix('\'')) {
        return inner;
    }
    if let Some(inner) = value.strip_prefix('"').and_then(|text| text.strip_suffix('"')) {
        return inner;
    }

    value
}

/// 去掉 `//` 与 `/* ... */` 注释，保留字符串里的斜杠。
fn strip_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_block = false;
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        if in_block {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }

        if let Some(active) = quote {
            out.push(ch);
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if ch == active || ch == '\n' {
                quote = None;
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            // 跳过到行尾，但保留换行，行号仍然对得上。
            for next in chars.by_ref() {
                if next == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block = true;
            continue;
        }

        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
        out.push(ch);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kotlin_dsl() {
        let config = parse(
            r#"
android {
    namespace = "com.example.demo"
    compileSdk = flutter.compileSdkVersion

    defaultConfig {
        applicationId = "com.example.demo"
        minSdk = flutter.minSdkVersion
        targetSdk = 35
    }
}
"#,
        );

        assert_eq!(config.namespace.as_deref(), Some("com.example.demo"));
        assert_eq!(config.application_id.as_deref(), Some("com.example.demo"));
        assert_eq!(config.compile_sdk.as_deref(), Some("flutter.compileSdkVersion"));
        assert_eq!(config.min_sdk.as_deref(), Some("flutter.minSdkVersion"));
        assert_eq!(config.target_sdk.as_deref(), Some("35"));
    }

    #[test]
    fn parses_groovy_dsl() {
        let config = parse(
            r#"
android {
    namespace 'com.example.groovy'
    compileSdkVersion 34
    defaultConfig {
        applicationId "com.example.groovy"
        minSdkVersion 21
        targetSdkVersion 34 // 行内注释
    }
}
"#,
        );

        assert_eq!(config.namespace.as_deref(), Some("com.example.groovy"));
        assert_eq!(config.application_id.as_deref(), Some("com.example.groovy"));
        assert_eq!(config.compile_sdk.as_deref(), Some("34"));
        assert_eq!(config.min_sdk.as_deref(), Some("21"));
        assert_eq!(config.target_sdk.as_deref(), Some("34"));
    }

    #[test]
    fn ignores_comments_and_similar_keys() {
        let config = parse(
            r#"
// applicationId "com.example.commented"
/* minSdk 1 */
android {
    applicationIdSuffix ".debug"
    minSdk = 26
}
"#,
        );

        assert_eq!(config.application_id, None);
        assert_eq!(config.min_sdk.as_deref(), Some("26"));
    }

    #[test]
    fn keeps_first_non_empty_value() {
        let config = parse("applicationId \"com.example.first\"\napplicationId \"com.example.second\"\n");

        assert_eq!(config.application_id.as_deref(), Some("com.example.first"));
    }
}
