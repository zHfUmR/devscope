//! 从 iOS 工程文件中提取部署目标、Bundle Identifier 等信息。
//!
//! Flutter 的 iOS 配置分散在 `Podfile` 和 `project.pbxproj` 里。两者都不是稳定
//! 格式，这里只做行级匹配：找到就展示，找不到就交给上层记为「缺失」。

/// 从 Podfile 中读取 `platform :ios, '12.0'` 的部署目标。
pub(super) fn parse_podfile_platform(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = strip_podfile_comment(line).trim();
        let rest = line.strip_prefix("platform :ios")?;
        let value = rest.trim().trim_start_matches(',').trim();
        let value = value.trim_matches(['\'', '"']).trim();

        (!value.is_empty()).then(|| value.to_string())
    })
}

/// 从 `project.pbxproj` 中读取 Runner 的 Bundle Identifier。
///
/// 文件里每个 target / configuration 都会有一份 `PRODUCT_BUNDLE_IDENTIFIER`，
/// 优先返回第一个不含 `Tests` 的非变量值。
pub(super) fn parse_bundle_identifier(content: &str) -> Option<String> {
    parse_assignments(content, "PRODUCT_BUNDLE_IDENTIFIER")
        .into_iter()
        .find(|value| !value.contains("Tests") && !value.contains("$("))
}

/// 从 `project.pbxproj` 中读取 `IPHONEOS_DEPLOYMENT_TARGET`。
pub(super) fn parse_deployment_target(content: &str) -> Option<String> {
    parse_assignments(content, "IPHONEOS_DEPLOYMENT_TARGET")
        .into_iter()
        .find(|value| !value.contains("$("))
}

/// 读取形如 `KEY = value;` 的赋值，去掉引号和行尾注释。
fn parse_assignments(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(key)?.trim_start();
            let rest = rest.strip_prefix('=')?.trim();
            // pbxproj 的赋值以分号结束，后面可能跟着 `/* Runner */` 注释。
            let value = rest.split(';').next().unwrap_or(rest).trim();
            let value = value.trim_matches('"').trim();

            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

fn strip_podfile_comment(line: &str) -> &str {
    match line.find('#') {
        Some(index) => &line[..index],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_podfile_platform() {
        assert_eq!(parse_podfile_platform("platform :ios, '12.0'\n").as_deref(), Some("12.0"));
        assert_eq!(parse_podfile_platform("  platform :ios, \"15.0\" # 注释\n").as_deref(), Some("15.0"));
        assert_eq!(parse_podfile_platform("platform :osx, '10.14'\n"), None);
    }

    #[test]
    fn parses_bundle_identifier_and_skips_test_targets() {
        let pbxproj = r#"
                PRODUCT_BUNDLE_IDENTIFIER = com.example.demo.RunnerTests;
                PRODUCT_BUNDLE_IDENTIFIER = com.example.demo;
                PRODUCT_BUNDLE_IDENTIFIER = com.example.demo;
"#;

        assert_eq!(parse_bundle_identifier(pbxproj).as_deref(), Some("com.example.demo"));
    }

    #[test]
    fn parses_deployment_target() {
        let pbxproj = r#"
                IPHONEOS_DEPLOYMENT_TARGET = 13.0;
                IPHONEOS_DEPLOYMENT_TARGET = 12.0;
"#;

        assert_eq!(parse_deployment_target(pbxproj).as_deref(), Some("13.0"));
    }

    #[test]
    fn ignores_variable_values_and_missing_keys() {
        let pbxproj = "PRODUCT_BUNDLE_IDENTIFIER = \"$(PRODUCT_BUNDLE_IDENTIFIER)\";";

        assert_eq!(parse_bundle_identifier(pbxproj), None);
        assert_eq!(parse_bundle_identifier("nothing here"), None);
    }
}
