//! Flutter 项目配置分析。
//!
//! 只读项目内的配置文件：`pubspec.yaml`、`.metadata`、Android 的 Gradle 文件、
//! iOS 的 Podfile 与 `project.pbxproj`。不执行 `flutter pub get`，不修改项目。
//! 所有解析逻辑都拆成纯函数（见子模块），分析入口只负责读写和组装。

use std::fs;
use std::io;
use std::path::Path;

use crate::models::project::{
    AndroidProjectConfig, IosProjectConfig, PlatformKind, ProjectAnalysis, ProjectMetadata, ProjectPlatform,
    ProjectStatus,
};
use crate::utils::fs::read_text_capped;

mod gradle;
mod ios_project;
mod pubspec;

/// pubspec.yaml 一般只有几 KB；上限用于防止误选超大文件。
const MAX_PUBSPEC_BYTES: usize = 1024 * 1024;

/// Gradle 文件同样很小，保留 1 MiB 上限。
const MAX_GRADLE_BYTES: usize = 1024 * 1024;

/// `project.pbxproj` 会随 target 增长，但通常远小于 4 MiB。
const MAX_PBXPROJ_BYTES: usize = 4 * 1024 * 1024;

/// Podfile / .metadata / Info.plist 都是小文件。
const MAX_SMALL_CONFIG_BYTES: usize = 256 * 1024;

/// 平台目录与展示名的固定顺序。
const PLATFORM_DIRS: [(PlatformKind, &str); 6] = [
    (PlatformKind::Android, "android"),
    (PlatformKind::Ios, "ios"),
    (PlatformKind::Web, "web"),
    (PlatformKind::Macos, "macos"),
    (PlatformKind::Linux, "linux"),
    (PlatformKind::Windows, "windows"),
];

/// 分析一个 Flutter 项目目录。
///
/// 业务失败（不是目录、缺少 pubspec）会体现在 `status` / `message`，
/// 不会返回 `Err`；调用方只处理真正的机制性失败。
pub fn analyze(path: &Path) -> ProjectAnalysis {
    let root_path = display_path(path);

    if path.as_os_str().is_empty() {
        return failed_project(root_path, "没有提供项目目录。".to_string());
    }

    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return failed_project(root_path, "所选路径不是目录，请选择一个 Flutter 项目目录。".to_string());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let message = format!("目录不存在：{root_path}");
            return failed_project(root_path, message);
        }
        Err(error) => {
            return failed_project(root_path, format!("无法访问项目目录：{error}"));
        }
    }

    let pubspec_text = match read_text_capped(&path.join("pubspec.yaml"), MAX_PUBSPEC_BYTES) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return failed_project(root_path, "该目录下没有 pubspec.yaml，可能不是 Flutter 项目。".to_string());
        }
        Err(error) => {
            return failed_project(root_path, format!("读取 pubspec.yaml 失败：{error}"));
        }
    };

    let pubspec = pubspec::parse_pubspec(&pubspec_text.text);
    let platforms = discover_platforms(path);
    let android_enabled = platforms.iter().any(|platform| platform.platform == PlatformKind::Android);
    let ios_enabled = platforms.iter().any(|platform| platform.platform == PlatformKind::Ios);

    let android = analyze_android(path, android_enabled);
    let ios = analyze_ios(path, ios_enabled);
    let metadata = analyze_metadata(path);

    let asset_count = pubspec.assets.len();
    let issues = evaluate_issues(&pubspec, &platforms, android.as_ref(), ios.as_ref(), pubspec_text.truncated);
    let status = if issues.is_empty() {
        ProjectStatus::Ok
    } else {
        ProjectStatus::Warning
    };

    ProjectAnalysis {
        status,
        root_path,
        name: pubspec.name,
        description: pubspec.description,
        version: pubspec.version,
        publish_to: pubspec.publish_to,
        dart_sdk_constraint: pubspec.dart_sdk,
        flutter_constraint: pubspec.flutter_sdk,
        dependencies: pubspec.dependencies,
        dev_dependencies: pubspec.dev_dependencies,
        dependency_overrides: pubspec.dependency_overrides,
        platforms,
        uses_material_design: pubspec.uses_material_design,
        asset_count,
        assets: pubspec.assets,
        font_families: pubspec.font_families,
        android,
        ios,
        metadata,
        issues,
        message: None,
    }
}

/// 收集实际存在的平台目录。
fn discover_platforms(root: &Path) -> Vec<ProjectPlatform> {
    PLATFORM_DIRS
        .iter()
        .filter(|(_, directory)| root.join(directory).is_dir())
        .map(|(platform, directory)| ProjectPlatform {
            platform: *platform,
            directory: (*directory).to_string(),
        })
        .collect()
}

/// 读取 Android 模块的 Gradle 配置。Kotlin DSL 优先，因为新项目默认使用它。
fn analyze_android(root: &Path, enabled: bool) -> Option<AndroidProjectConfig> {
    if !enabled {
        return None;
    }

    for relative in ["android/app/build.gradle.kts", "android/app/build.gradle"] {
        let Ok(text) = read_text_capped(&root.join(relative), MAX_GRADLE_BYTES) else {
            continue;
        };
        let config = gradle::parse(&text.text);

        return Some(AndroidProjectConfig {
            gradle_file: relative.to_string(),
            kotlin_dsl: relative.ends_with(".kts"),
            application_id: config.application_id,
            namespace: config.namespace,
            compile_sdk: config.compile_sdk,
            min_sdk: config.min_sdk,
            target_sdk: config.target_sdk,
        });
    }

    None
}

/// 读取 iOS 工程配置。即使文件缺失也返回结构，让 UI 展示「缺失」而不是整页空白。
fn analyze_ios(root: &Path, enabled: bool) -> Option<IosProjectConfig> {
    if !enabled {
        return None;
    }

    let podfile_path = root.join("ios/Podfile");
    let pbxproj_path = root.join("ios/Runner.xcodeproj/project.pbxproj");

    let podfile = read_optional(&podfile_path, MAX_SMALL_CONFIG_BYTES);
    let pbxproj = read_optional(&pbxproj_path, MAX_PBXPROJ_BYTES);

    let deployment_target = podfile
        .as_deref()
        .and_then(ios_project::parse_podfile_platform)
        .or_else(|| pbxproj.as_deref().and_then(ios_project::parse_deployment_target));

    Some(IosProjectConfig {
        deployment_target,
        bundle_identifier: pbxproj.as_deref().and_then(ios_project::parse_bundle_identifier),
        podfile_present: podfile_path.is_file(),
        workspace_present: root.join("ios/Runner.xcworkspace").is_dir(),
        info_plist_present: root.join("ios/Runner/Info.plist").is_file(),
    })
}

fn analyze_metadata(root: &Path) -> Option<ProjectMetadata> {
    let text = read_optional(&root.join(".metadata"), MAX_SMALL_CONFIG_BYTES)?;
    let metadata = pubspec::parse_metadata(&text);

    if metadata.channel.is_none() && metadata.revision.is_none() && metadata.project_type.is_none() {
        return None;
    }

    Some(metadata)
}

fn read_optional(path: &Path, limit: usize) -> Option<String> {
    read_text_capped(path, limit).ok().map(|text| text.text)
}

/// 根据已解析的字段判断需要提醒用户什么。这里只产生「可修复」的问题。
fn evaluate_issues(
    pubspec: &pubspec::Pubspec,
    platforms: &[ProjectPlatform],
    android: Option<&AndroidProjectConfig>,
    ios: Option<&IosProjectConfig>,
    pubspec_truncated: bool,
) -> Vec<String> {
    let mut issues = Vec::new();

    if pubspec.name.is_none() {
        issues.push("pubspec.yaml 缺少 name 字段，Flutter 无法识别项目名。".to_string());
    }
    if pubspec.dependencies.is_empty() {
        issues.push("pubspec.yaml 没有声明 dependencies。".to_string());
    }
    if pubspec.dart_sdk.is_none() {
        issues.push("pubspec.yaml 未声明 Dart SDK 约束（environment.sdk）。".to_string());
    }
    if pubspec_truncated {
        issues.push("pubspec.yaml 较大，只读取了前 1 MiB，分析结果可能不完整。".to_string());
    }
    if platforms.is_empty() {
        issues.push("未找到 android/ios/web 等平台目录，项目可能尚未启用任何平台。".to_string());
    }

    if platforms.iter().any(|platform| platform.platform == PlatformKind::Android) {
        match android {
            Some(config) if config.application_id.is_none() => {
                issues.push("Android 配置中未找到 applicationId。".to_string());
            }
            None => {
                issues.push("未找到 android/app/build.gradle(.kts)。".to_string());
            }
            Some(_) => {}
        }
    }

    if platforms.iter().any(|platform| platform.platform == PlatformKind::Ios)
        && let Some(config) = ios
    {
        if config.bundle_identifier.is_none() {
            issues.push("iOS 工程中未找到 Runner 的 Bundle Identifier。".to_string());
        }
        if config.deployment_target.is_none() {
            issues.push("iOS 工程中未找到部署目标（IPHONEOS_DEPLOYMENT_TARGET）。".to_string());
        }
        if !config.podfile_present {
            issues.push("未找到 ios/Podfile，含插件的项目可能无法正常构建 iOS。".to_string());
        }
    }

    issues
}

fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();

    if text.len() > 1 {
        text.trim_end_matches('/').to_string()
    } else {
        text.into_owned()
    }
}

fn failed_project(root_path: String, message: String) -> ProjectAnalysis {
    ProjectAnalysis {
        status: ProjectStatus::Error,
        root_path,
        name: None,
        description: None,
        version: None,
        publish_to: None,
        dart_sdk_constraint: None,
        flutter_constraint: None,
        dependencies: Vec::new(),
        dev_dependencies: Vec::new(),
        dependency_overrides: Vec::new(),
        platforms: Vec::new(),
        uses_material_design: false,
        asset_count: 0,
        assets: Vec::new(),
        font_families: Vec::new(),
        android: None,
        ios: None,
        metadata: None,
        issues: Vec::new(),
        message: Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 临时项目目录，Drop 时递归清理。
    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!("devscope-project-test-{label}-{}-{id}", std::process::id()));
            fs::create_dir_all(&root).unwrap();

            Self { root }
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    const PUBSPEC: &str = r#"
name: demo_app
description: A demo project
version: 1.2.3+4
publish_to: 'none'

environment:
  sdk: ^3.5.0
  flutter: ">=3.24.0"

dependencies:
  flutter:
    sdk: flutter
  http: ^1.2.0

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  uses-material-design: true
  assets:
    - assets/logo.png
"#;

    fn write_complete_project(project: &TempProject) {
        project.write("pubspec.yaml", PUBSPEC);
        project.write(".metadata", "version:\n  revision: abc123\n  channel: stable\nproject_type: app\n");
        project.write(
            "android/app/build.gradle.kts",
            r#"
android {
    namespace = "com.example.demo"
    compileSdk = flutter.compileSdkVersion
    defaultConfig {
        applicationId = "com.example.demo"
        minSdk = 24
        targetSdk = 35
    }
}
"#,
        );
        project.write("ios/Podfile", "platform :ios, '13.0'\n");
        project.write("ios/Runner.xcodeproj/project.pbxproj", "PRODUCT_BUNDLE_IDENTIFIER = com.example.demo;\n");
        project.write("ios/Runner/Info.plist", "<plist/>");
    }

    #[test]
    fn analyzes_complete_flutter_project() {
        let project = TempProject::new("complete");
        write_complete_project(&project);

        let analysis = analyze(&project.root);

        assert_eq!(analysis.status, ProjectStatus::Ok, "issues: {:?}", analysis.issues);
        assert_eq!(analysis.name.as_deref(), Some("demo_app"));
        assert_eq!(analysis.version.as_deref(), Some("1.2.3+4"));
        assert_eq!(analysis.dependencies.len(), 2);
        assert!(analysis.uses_material_design);
        assert_eq!(analysis.asset_count, 1);
        assert_eq!(analysis.platforms.len(), 2);

        let android = analysis.android.expect("应该有 Android 配置");
        assert!(android.kotlin_dsl);
        assert_eq!(android.application_id.as_deref(), Some("com.example.demo"));
        assert_eq!(android.min_sdk.as_deref(), Some("24"));
        assert_eq!(android.target_sdk.as_deref(), Some("35"));

        let ios = analysis.ios.expect("应该有 iOS 配置");
        assert_eq!(ios.deployment_target.as_deref(), Some("13.0"));
        assert_eq!(ios.bundle_identifier.as_deref(), Some("com.example.demo"));
        assert!(ios.podfile_present);

        let metadata = analysis.metadata.expect("应该有 .metadata");
        assert_eq!(metadata.channel.as_deref(), Some("stable"));
    }

    #[test]
    fn reports_missing_pubspec() {
        let project = TempProject::new("empty");

        let analysis = analyze(&project.root);

        assert_eq!(analysis.status, ProjectStatus::Error);
        assert!(analysis.message.unwrap_or_default().contains("pubspec.yaml"));
    }

    #[test]
    fn warns_when_platforms_and_config_are_missing() {
        let project = TempProject::new("minimal");
        project.write("pubspec.yaml", "name: minimal\nenvironment:\n  sdk: ^3.5.0\n");

        let analysis = analyze(&project.root);

        assert_eq!(analysis.status, ProjectStatus::Warning);
        assert!(analysis.issues.iter().any(|issue| issue.contains("平台目录")), "issues: {:?}", analysis.issues);
        assert!(analysis.issues.iter().any(|issue| issue.contains("dependencies")), "issues: {:?}", analysis.issues);
    }

    #[test]
    fn rejects_non_directory_path() {
        let project = TempProject::new("file");
        project.write("pubspec.yaml", PUBSPEC);

        let analysis = analyze(&project.root.join("pubspec.yaml"));

        assert_eq!(analysis.status, ProjectStatus::Error);
        assert!(analysis.message.unwrap_or_default().contains("不是目录"));
    }
}
