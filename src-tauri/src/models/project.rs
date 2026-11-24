//! Flutter 项目分析的数据契约。
//!
//! 项目分析只读取项目内的配置文件（pubspec、Gradle、Podfile、pbxproj），
//! 不执行 `flutter pub get`，也不修改任何文件。字段经 serde 转为 camelCase，
//! 与前端 TS 一一对应。

use serde::{Deserialize, Serialize};

/// 一次项目分析的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectStatus {
    /// 所有关键配置都能读取并解析。
    Ok,
    /// 是 Flutter 项目，但缺少某些配置，仍可继续开发。
    Warning,
    /// 不是目录、没有 pubspec.yaml，或配置无法读取。
    Error,
}

/// 依赖在 pubspec.yaml 中属于哪一组。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    /// dependencies。
    Regular,
    /// dev_dependencies。
    Dev,
    /// dependency_overrides。
    Override,
}

/// 依赖的来源类型，用于解释约束字符串的含义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencySource {
    /// 来自 pub.dev（普通版本约束）。
    Hosted,
    /// Flutter / Dart SDK 内置包，例如 `sdk: flutter`。
    Sdk,
    /// Git 依赖。
    Git,
    /// 本地路径依赖。
    Path,
    /// 无法识别来源。
    Unknown,
}

/// pubspec.yaml 中声明的一个依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDependency {
    /// 包名。
    pub name: String,
    /// 版本约束或来源描述；`any` 等空约束为 None。
    pub constraint: Option<String>,
    /// 属于 dependencies / dev_dependencies / dependency_overrides。
    pub kind: DependencyKind,
    /// 依赖来源类型。
    pub source: DependencySource,
}

/// 项目启用的目标平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlatformKind {
    Android,
    Ios,
    Web,
    Macos,
    Linux,
    Windows,
}

/// 一个已存在的平台目录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlatform {
    /// 平台标识。
    pub platform: PlatformKind,
    /// 相对项目根目录的目录名，例如 `android`。
    pub directory: String,
}

/// Android 模块的关键 Gradle 配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidProjectConfig {
    /// 读取的 Gradle 文件，相对项目根目录。
    pub gradle_file: String,
    /// 是否为 Kotlin DSL（`build.gradle.kts`）。
    pub kotlin_dsl: bool,
    /// 应用包名。
    pub application_id: Option<String>,
    /// Gradle namespace。
    pub namespace: Option<String>,
    /// 编译 SDK；可能是 `flutter.compileSdkVersion` 这类表达式。
    pub compile_sdk: Option<String>,
    /// 最低 SDK。
    pub min_sdk: Option<String>,
    /// 目标 SDK。
    pub target_sdk: Option<String>,
}

/// iOS 模块的关键配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosProjectConfig {
    /// 部署目标，例如 `12.0`。
    pub deployment_target: Option<String>,
    /// Runner 的 Bundle Identifier。
    pub bundle_identifier: Option<String>,
    /// 是否存在 `ios/Podfile`。
    pub podfile_present: bool,
    /// 是否存在 `ios/Runner.xcworkspace`。
    pub workspace_present: bool,
    /// 是否存在 `ios/Runner/Info.plist`。
    pub info_plist_present: bool,
}

/// `.metadata` 文件中的 Flutter 工具信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    /// 发布渠道。
    pub channel: Option<String>,
    /// Git revision。
    pub revision: Option<String>,
    /// 项目类型，例如 app / package / plugin。
    pub project_type: Option<String>,
}

/// 一次 Flutter 项目分析的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAnalysis {
    /// 业务结论。
    pub status: ProjectStatus,
    /// 用户选择的项目根目录。
    pub root_path: String,
    /// pubspec.yaml 中的包名。
    pub name: Option<String>,
    /// 项目描述。
    pub description: Option<String>,
    /// 项目版本，例如 `1.0.0+1`。
    pub version: Option<String>,
    /// `publish_to`，`none` 表示不发布到 pub.dev。
    pub publish_to: Option<String>,
    /// Dart SDK 约束。
    pub dart_sdk_constraint: Option<String>,
    /// Flutter SDK 约束。
    pub flutter_constraint: Option<String>,
    /// dependencies。
    pub dependencies: Vec<ProjectDependency>,
    /// dev_dependencies。
    pub dev_dependencies: Vec<ProjectDependency>,
    /// dependency_overrides。
    pub dependency_overrides: Vec<ProjectDependency>,
    /// 已存在的平台目录。
    pub platforms: Vec<ProjectPlatform>,
    /// 是否启用 Material 设计。
    pub uses_material_design: bool,
    /// `flutter.assets` 中声明的资源数量。
    pub asset_count: usize,
    /// `flutter.assets` 中声明的资源路径。
    #[serde(default)]
    pub assets: Vec<String>,
    /// `flutter.fonts` 中声明的字体家族。
    pub font_families: Vec<String>,
    /// Android 模块配置；没有 android 目录时为 None。
    pub android: Option<AndroidProjectConfig>,
    /// iOS 模块配置；没有 ios 目录时为 None。
    pub ios: Option<IosProjectConfig>,
    /// `.metadata` 中的工具信息。
    pub metadata: Option<ProjectMetadata>,
    /// 需要用户关注但不会中断分析的问题。
    pub issues: Vec<String>,
    /// 无法分析时的原因。
    pub message: Option<String>,
}
