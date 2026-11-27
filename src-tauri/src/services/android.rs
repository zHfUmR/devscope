//! Android 开发环境检测：SDK 目录、JDK、platform、build-tools 和模拟器。
//!
//! 检测只读文件系统，不执行 `adb` / `emulator`，避免启动后台进程或产生副作用。
//! 因此这一层可以被 `cargo test` 完全覆盖，不要求本机真的安装 Android SDK。

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::models::android::{AndroidEnvironment, AndroidSdkSource, AndroidStatus, JavaRuntime, JavaSource};
use crate::utils::executable;
use crate::utils::fs::{home_dir, is_executable};
use crate::utils::process;
use crate::utils::version::compare_versions;

/// Android SDK 根目录可能通过这两个环境变量提供。
const SDK_ENV_VARS: [&str; 2] = ["ANDROID_HOME", "ANDROID_SDK_ROOT"];

/// `platforms/` 目录下的 platform 名称前缀。
const PLATFORM_PREFIX: &str = "android-";

/// 一个待验证的 SDK 候选目录。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SdkCandidate {
    path: PathBuf,
    source: AndroidSdkSource,
}

impl executable::PathCandidate for SdkCandidate {
    fn path(&self) -> &Path {
        &self.path
    }
}

/// SDK 路径发现结果：命中项，以及本次检查过的全部路径。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SdkDiscovery {
    candidate: Option<SdkCandidate>,
    checked_paths: Vec<String>,
}

/// 从一个 SDK 目录里读出的静态结构。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SdkLayout {
    platforms: Vec<String>,
    build_tools: Vec<String>,
    cmdline_tools_version: Option<String>,
    sdk_manager_path: Option<PathBuf>,
    adb_path: Option<PathBuf>,
    emulator_path: Option<PathBuf>,
    accepted_licenses: usize,
}

/// 一个待验证的 JDK 候选目录。
#[derive(Debug, Clone, PartialEq, Eq)]
struct JavaCandidate {
    path: PathBuf,
    source: JavaSource,
}

impl executable::PathCandidate for JavaCandidate {
    fn path(&self) -> &Path {
        &self.path
    }
}

/// 检测本机 Android 环境，是 services 层对外的唯一入口。
///
/// 业务失败会体现在返回值的 `status` / `message` / `issues` 上，而不是 `Err`。
pub fn detect() -> AndroidEnvironment {
    detect_with(discover_sdk())
}

/// 用给定的 SDK 发现结果执行检测；测试可以注入候选目录。
fn detect_with(discovery: SdkDiscovery) -> AndroidEnvironment {
    let SdkDiscovery {
        candidate,
        checked_paths,
    } = discovery;

    let Some(candidate) = candidate else {
        return not_found_environment(checked_paths);
    };

    let sdk_path = candidate.path.to_string_lossy().into_owned();
    let layout = match inspect_sdk(&candidate.path) {
        Ok(layout) => layout,
        Err(error) => {
            return failed_environment(
                sdk_path,
                candidate.source,
                format!("找到 Android SDK（{}），但读取目录失败：{error}。", candidate.path.display()),
                checked_paths,
            );
        }
    };

    let java = discover_java();
    let avds = discover_avds(home_dir().as_deref());

    build_environment(candidate, layout, java, avds, checked_paths)
}

/// 把已读到的 SDK 结构、JDK 和 AVD 组合成最终模型。
///
/// 单独拆出来是为了测试可以注入固定的 JDK / AVD，不依赖本机安装情况。
fn build_environment(
    candidate: SdkCandidate,
    layout: SdkLayout,
    java: Option<JavaRuntime>,
    avds: Vec<String>,
    checked_paths: Vec<String>,
) -> AndroidEnvironment {
    let issues = evaluate_issues(&layout, java.is_some());
    let status = if issues.is_empty() {
        AndroidStatus::Ok
    } else {
        AndroidStatus::Warning
    };

    AndroidEnvironment {
        status,
        sdk_path: Some(candidate.path.to_string_lossy().into_owned()),
        sdk_source: Some(candidate.source),
        platforms: layout.platforms,
        build_tools: layout.build_tools,
        cmdline_tools_version: layout.cmdline_tools_version,
        sdk_manager_path: layout.sdk_manager_path.map(|path| path.to_string_lossy().into_owned()),
        adb_path: layout.adb_path.map(|path| path.to_string_lossy().into_owned()),
        emulator_path: layout.emulator_path.map(|path| path.to_string_lossy().into_owned()),
        avds,
        java,
        accepted_licenses: layout.accepted_licenses,
        issues,
        message: None,
        checked_paths,
    }
}

/// 根据 SDK 结构判断还需要提醒用户什么。
///
/// 这里只产生“可修复”的问题，所以对应 `Warning`；真正的读取失败在调用方处理。
fn evaluate_issues(layout: &SdkLayout, has_java: bool) -> Vec<String> {
    let mut issues = Vec::new();

    if layout.platforms.is_empty() {
        issues.push("未安装 Android Platform（platforms 目录为空）。".to_string());
    }
    if layout.build_tools.is_empty() {
        issues.push("未安装 Android Build Tools（build-tools 目录为空）。".to_string());
    }
    if !has_java {
        issues.push("未找到 JDK：Flutter 构建 Android 应用需要 Java。".to_string());
    }
    if layout.accepted_licenses == 0 {
        issues.push("尚未接受 Android SDK licenses，构建时可能被拒绝。".to_string());
    }
    if layout.adb_path.is_none() {
        issues.push("未找到 platform-tools/adb，无法连接 Android 设备。".to_string());
    }
    if layout.emulator_path.is_none() {
        issues.push("未找到 emulator，无法启动 Android 模拟器（仍可使用真机）。".to_string());
    }
    if layout.sdk_manager_path.is_none() {
        issues.push("未找到 cmdline-tools/sdkmanager，无法在应用内管理 SDK 组件。".to_string());
    }

    issues
}

/// 按固定优先级查找 SDK：环境变量 → 常见安装目录。
///
/// 环境变量优先，因为用户可能把 SDK 装在非标准位置；常见目录覆盖
/// Android Studio 的默认安装路径。
fn discover_sdk() -> SdkDiscovery {
    let home = home_dir();
    let env_home = env::var_os(SDK_ENV_VARS[0]);
    let env_root = env::var_os(SDK_ENV_VARS[1]);

    discover_sdk_with(home.as_deref(), env_home.as_deref(), env_root.as_deref())
}

/// 用显式参数执行 SDK 发现，便于测试不修改进程级环境变量。
fn discover_sdk_with(home: Option<&Path>, env_home: Option<&OsStr>, env_root: Option<&OsStr>) -> SdkDiscovery {
    let mut candidates = Vec::new();

    for value in [env_home, env_root].into_iter().flatten() {
        if !value.is_empty() {
            executable::push_unique(
                &mut candidates,
                SdkCandidate {
                    path: PathBuf::from(value),
                    source: AndroidSdkSource::Environment,
                },
            );
        }
    }

    candidates.extend(common_sdk_candidates(home));

    // 环境变量可能正好指向常见目录，去重后再生成 checked_paths。
    let mut unique = Vec::new();
    for candidate in candidates {
        executable::push_unique(&mut unique, candidate);
    }
    let candidates = unique;

    let candidate = candidates.iter().find(|candidate| candidate.path.is_dir()).cloned();
    let checked_paths = candidates
        .iter()
        .map(|candidate| candidate.path.to_string_lossy().into_owned())
        .collect();

    SdkDiscovery {
        candidate,
        checked_paths,
    }
}

/// macOS 上常见的 Android SDK 安装目录。
fn common_sdk_candidates(home: Option<&Path>) -> Vec<SdkCandidate> {
    let mut paths = Vec::new();

    if let Some(home) = home {
        paths.push(home.join("Library/Android/sdk"));
        paths.push(home.join("Android/Sdk"));
    }

    paths.push(PathBuf::from("/opt/homebrew/share/android-commandlinetools"));
    paths.push(PathBuf::from("/usr/local/share/android-commandlinetools"));

    paths
        .into_iter()
        .map(|path| SdkCandidate {
            path,
            source: AndroidSdkSource::CommonLocation,
        })
        .collect()
}

/// 读取 SDK 根目录下的关键子目录和可执行文件。
///
/// 根目录不可读时返回 `Err`；子目录缺失属于正常情况，返回空列表，
/// 由 [`evaluate_issues`] 转成用户能看懂的提示。
fn inspect_sdk(root: &Path) -> io::Result<SdkLayout> {
    // 先确认根目录可读，避免把「权限不足」误判成「组件没装」。
    fs::read_dir(root)?;

    let platforms = list_version_dirs(&root.join("platforms"), PLATFORM_PREFIX);
    let build_tools = list_version_dirs(&root.join("build-tools"), "");
    let cmdline_tools_version = cmdline_tools_version(&root.join("cmdline-tools"));

    let sdk_manager_path = cmdline_tools_version
        .as_deref()
        .map(|version| root.join("cmdline-tools").join(version).join("bin/sdkmanager"))
        .filter(|path| is_executable(path))
        .or_else(|| {
            let legacy = root.join("tools/bin/sdkmanager");
            is_executable(&legacy).then_some(legacy)
        });

    let adb_path = root.join("platform-tools/adb");
    let emulator_path = root.join("emulator/emulator");

    Ok(SdkLayout {
        platforms,
        build_tools,
        cmdline_tools_version,
        sdk_manager_path,
        adb_path: is_executable(&adb_path).then_some(adb_path),
        emulator_path: is_executable(&emulator_path).then_some(emulator_path),
        accepted_licenses: count_license_files(&root.join("licenses")),
    })
}

/// 列出目录下以 `prefix` 开头的子目录，并按版本从新到旧排序。
fn list_version_dirs(root: &Path, prefix: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            let rest = name.strip_prefix(prefix)?;
            (!rest.is_empty()).then_some(name)
        })
        .collect();

    names.sort_by(|a, b| compare_versions(b, a));
    names
}

/// `cmdline-tools` 优先使用 `latest`，否则取版本号最大的目录。
fn cmdline_tools_version(root: &Path) -> Option<String> {
    let versions = list_version_dirs(root, "");
    if versions.iter().any(|version| version == "latest") {
        return Some("latest".to_string());
    }

    versions.into_iter().next()
}

/// 统计 `licenses/` 下已接受的 license 文件数量。
///
/// SDK 的 license 接受状态就是这些哈希文件；只数文件，不读取内容。
fn count_license_files(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };

    entries.flatten().filter(|entry| entry.path().is_file()).count()
}

/// 从 JDK 的 `release` 文件读取 `JAVA_VERSION`。
///
/// Android Studio 自带的 JBR 和标准 JDK 都会生成这个文件，读取它比启动
/// `java -version` 更快，也不需要处理进程输出。
fn read_java_release(home: &Path) -> Option<String> {
    let text = fs::read_to_string(home.join("release")).ok()?;

    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim() != "JAVA_VERSION" {
            return None;
        }

        let version = value.trim().trim_matches('"');
        (!version.is_empty()).then(|| version.to_string())
    })
}

/// 查找一个可用的 JDK：`JAVA_HOME` → Android Studio JBR → 系统 JDK。
fn discover_java() -> Option<JavaRuntime> {
    let home = home_dir();
    let mut candidates = java_candidates(home.as_deref());

    // GUI 进程可能没有终端里的 JAVA_HOME；macOS 自带的 java_home 是最后兜底。
    if let Some(path) = macos_java_home() {
        push_java_candidate(&mut candidates, path, JavaSource::System);
    }

    candidates.into_iter().find_map(|candidate| {
        if !is_executable(&candidate.path.join("bin/java")) {
            return None;
        }

        Some(JavaRuntime {
            version: read_java_release(&candidate.path),
            path: candidate.path.to_string_lossy().into_owned(),
            source: candidate.source,
        })
    })
}

/// 通过 macOS 的 `/usr/libexec/java_home` 读取默认 JDK 路径。
///
/// 它覆盖「JDK 已注册到系统、但 GUI 进程没有 JAVA_HOME」的情况。
fn macos_java_home() -> Option<PathBuf> {
    let mut command = Command::new("/usr/libexec/java_home");
    let output = process::run_with_timeout(&mut command, Duration::from_secs(2)).ok()?;
    if !output.success() {
        return None;
    }

    let path = output.stdout.lines().map(str::trim).find(|line| !line.is_empty())?;

    let path = PathBuf::from(path);
    path.is_absolute().then_some(path)
}

/// 收集 macOS 上常见的 JDK 根目录，按优先级排列。
fn java_candidates(home: Option<&Path>) -> Vec<JavaCandidate> {
    let mut candidates = Vec::new();

    if let Some(value) = env::var_os("JAVA_HOME")
        && !value.is_empty()
    {
        push_java_candidate(&mut candidates, PathBuf::from(value), JavaSource::JavaHome);
    }

    if let Some(home) = home {
        push_java_candidate(
            &mut candidates,
            home.join("Applications/Android Studio.app/Contents/jbr/Contents/Home"),
            JavaSource::AndroidStudio,
        );
        push_java_candidate(&mut candidates, home.join(".sdkman/candidates/java/current"), JavaSource::System);
    }

    push_java_candidate(
        &mut candidates,
        PathBuf::from("/Applications/Android Studio.app/Contents/jbr/Contents/Home"),
        JavaSource::AndroidStudio,
    );
    push_java_candidate(
        &mut candidates,
        PathBuf::from("/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home"),
        JavaSource::System,
    );
    push_java_candidate(
        &mut candidates,
        PathBuf::from("/usr/local/opt/openjdk/libexec/openjdk.jdk/Contents/Home"),
        JavaSource::System,
    );

    scan_jvms(Path::new("/Library/Java/JavaVirtualMachines"), &mut candidates);
    if let Some(home) = home {
        scan_jvms(&home.join("Library/Java/JavaVirtualMachines"), &mut candidates);
    }

    candidates
}

/// 扫描 `JavaVirtualMachines/*/Contents/Home` 形式的 JDK。
fn scan_jvms(root: &Path, candidates: &mut Vec<JavaCandidate>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    let mut homes: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path().join("Contents/Home"))
        .filter(|home| home.is_dir())
        .collect();
    homes.sort();

    for home in homes {
        push_java_candidate(candidates, home, JavaSource::System);
    }
}

/// 把「路径 + 来源」组装成 JDK 候选并按路径去重。
fn push_java_candidate(candidates: &mut Vec<JavaCandidate>, path: PathBuf, source: JavaSource) {
    executable::push_unique(candidates, JavaCandidate { path, source });
}

/// 读取 `~/.android/avd/*.ini` 的文件名作为已配置的模拟器名称。
///
/// 不启动 `emulator -list-avds`，避免为了展示列表而拉起模拟器进程。
fn discover_avds(home: Option<&Path>) -> Vec<String> {
    let Some(home) = home else {
        return Vec::new();
    };

    let Ok(entries) = fs::read_dir(home.join(".android/avd")) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension() != Some(OsStr::new("ini")) {
                return None;
            }

            path.file_stem().map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();

    names.sort();
    names
}

/// 构造「未安装」结果，保留 checked_paths 供前端展开排查。
fn not_found_environment(checked_paths: Vec<String>) -> AndroidEnvironment {
    AndroidEnvironment {
        status: AndroidStatus::NotFound,
        sdk_path: None,
        sdk_source: None,
        platforms: Vec::new(),
        build_tools: Vec::new(),
        cmdline_tools_version: None,
        sdk_manager_path: None,
        adb_path: None,
        emulator_path: None,
        avds: Vec::new(),
        java: None,
        accepted_licenses: 0,
        issues: Vec::new(),
        message: Some(
            "未检测到 Android SDK。请安装 Android Studio，或设置 ANDROID_HOME / ANDROID_SDK_ROOT。".to_string(),
        ),
        checked_paths,
    }
}

/// 构造 SDK 目录读取失败的结果。
fn failed_environment(
    sdk_path: String,
    sdk_source: AndroidSdkSource,
    message: String,
    checked_paths: Vec<String>,
) -> AndroidEnvironment {
    AndroidEnvironment {
        status: AndroidStatus::Error,
        sdk_path: Some(sdk_path),
        sdk_source: Some(sdk_source),
        platforms: Vec::new(),
        build_tools: Vec::new(),
        cmdline_tools_version: None,
        sdk_manager_path: None,
        adb_path: None,
        emulator_path: None,
        avds: Vec::new(),
        java: None,
        accepted_licenses: 0,
        issues: Vec::new(),
        message: Some(message),
        checked_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 测试用临时目录，Drop 时自动清理。
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!("devscope-android-test-{label}-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// 在临时目录里创建一个具备执行权限的空文件。
    fn write_executable(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "#!/bin/sh\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    /// 构造一个完整的假 SDK 目录。
    fn create_sdk(root: &Path) {
        fs::create_dir_all(root.join("platforms/android-36")).unwrap();
        fs::create_dir_all(root.join("platforms/android-37.0")).unwrap();
        fs::create_dir_all(root.join("build-tools/36.0.0")).unwrap();
        fs::create_dir_all(root.join("cmdline-tools/latest/bin")).unwrap();
        fs::create_dir_all(root.join("licenses")).unwrap();
        fs::write(root.join("licenses/android-sdk-license"), "hash").unwrap();
        write_executable(&root.join("cmdline-tools/latest/bin/sdkmanager"));
        write_executable(&root.join("platform-tools/adb"));
        write_executable(&root.join("emulator/emulator"));
    }

    #[test]
    fn sorts_versions_numerically_descending() {
        let temp = TempDir::new("versions");
        fs::create_dir_all(temp.path().join("platforms/android-36")).unwrap();
        fs::create_dir_all(temp.path().join("platforms/android-37.0")).unwrap();
        fs::create_dir_all(temp.path().join("platforms/android-9")).unwrap();

        let versions = list_version_dirs(&temp.path().join("platforms"), PLATFORM_PREFIX);

        assert_eq!(versions, vec!["android-37.0", "android-36", "android-9"]);
    }

    #[test]
    fn inspects_complete_sdk_layout() {
        let temp = TempDir::new("layout");
        let root = temp.path();
        create_sdk(root);

        let layout = inspect_sdk(root).unwrap();

        assert_eq!(layout.platforms, vec!["android-37.0", "android-36"]);
        assert_eq!(layout.build_tools, vec!["36.0.0"]);
        assert_eq!(layout.cmdline_tools_version.as_deref(), Some("latest"));
        assert!(layout.sdk_manager_path.is_some());
        assert!(layout.adb_path.is_some());
        assert!(layout.emulator_path.is_some());
        assert_eq!(layout.accepted_licenses, 1);
        assert!(evaluate_issues(&layout, true).is_empty());
    }

    #[test]
    fn missing_components_produce_warnings() {
        let temp = TempDir::new("missing");
        let root = temp.path();
        fs::create_dir_all(root.join("platforms")).unwrap();
        fs::create_dir_all(root.join("build-tools")).unwrap();

        let layout = inspect_sdk(root).unwrap();
        let issues = evaluate_issues(&layout, false);

        assert_eq!(layout.accepted_licenses, 0);
        assert!(issues.iter().any(|issue| issue.contains("Platform")));
        assert!(issues.iter().any(|issue| issue.contains("Build Tools")));
        assert!(issues.iter().any(|issue| issue.contains("JDK")));
        assert!(issues.iter().any(|issue| issue.contains("licenses")));
        assert!(issues.iter().any(|issue| issue.contains("adb")));
    }

    #[test]
    fn reads_java_version_from_release_file() {
        let temp = TempDir::new("java");
        fs::write(temp.path().join("release"), "IMPLEMENTOR=\"JetBrains s.r.o.\"\nJAVA_VERSION=\"25.0.3\"\n").unwrap();

        assert_eq!(read_java_release(temp.path()), Some("25.0.3".to_string()));
    }

    #[test]
    fn ignores_incomplete_java_release_file() {
        let temp = TempDir::new("java-empty");
        fs::write(temp.path().join("release"), "IMPLEMENTOR=\"JetBrains\"\n").unwrap();

        assert_eq!(read_java_release(temp.path()), None);
    }

    #[test]
    fn discovers_avds_from_ini_files() {
        let temp = TempDir::new("avd");
        fs::create_dir_all(temp.path().join(".android/avd")).unwrap();
        fs::write(temp.path().join(".android/avd/Pixel_8.ini"), "avd.ini").unwrap();
        fs::write(temp.path().join(".android/avd/Pixel_9.ini"), "avd.ini").unwrap();
        fs::write(temp.path().join(".android/avd/notes.txt"), "ignore").unwrap();

        let avds = discover_avds(Some(temp.path()));

        assert_eq!(avds, vec!["Pixel_8", "Pixel_9"]);
    }

    #[test]
    fn prefers_environment_candidate_over_common_path() {
        let temp = TempDir::new("env");
        let env_sdk = temp.path().join("custom-sdk");
        fs::create_dir_all(&env_sdk).unwrap();

        let discovery = discover_sdk_with(Some(temp.path()), Some(env_sdk.as_os_str()), Some(OsStr::new("")));

        let candidate = discovery.candidate.unwrap();
        assert_eq!(candidate.path, env_sdk);
        assert_eq!(candidate.source, AndroidSdkSource::Environment);
    }

    #[test]
    fn deduplicates_environment_and_common_paths() {
        let temp = TempDir::new("dedupe");
        let sdk = temp.path().join("Library/Android/sdk");
        fs::create_dir_all(&sdk).unwrap();

        let discovery = discover_sdk_with(Some(temp.path()), Some(sdk.as_os_str()), Some(OsStr::new("")));
        let sdk_text = sdk.to_string_lossy();

        assert_eq!(discovery.candidate.unwrap().path, sdk);
        assert_eq!(
            discovery
                .checked_paths
                .iter()
                .filter(|path| path.as_str() == sdk_text.as_ref())
                .count(),
            1
        );
    }

    #[test]
    fn reports_not_found_without_candidates() {
        let environment = detect_with(SdkDiscovery {
            candidate: None,
            checked_paths: vec!["/missing/android-sdk".to_string()],
        });

        assert_eq!(environment.status, AndroidStatus::NotFound);
        assert_eq!(environment.checked_paths, vec!["/missing/android-sdk"]);
        assert!(environment.message.is_some());
    }

    #[test]
    fn builds_warning_when_java_is_missing() {
        let temp = TempDir::new("assessment");
        let root = temp.path();
        create_sdk(root);
        let layout = inspect_sdk(root).unwrap();

        let environment = build_environment(
            SdkCandidate {
                path: root.to_path_buf(),
                source: AndroidSdkSource::CommonLocation,
            },
            layout,
            None,
            Vec::new(),
            vec![root.to_string_lossy().into_owned()],
        );

        assert_eq!(environment.status, AndroidStatus::Warning);
        assert!(environment.issues.iter().any(|issue| issue.contains("JDK")));
        assert!(environment.message.is_none());
    }

    #[test]
    fn serializes_contract_in_camel_case() {
        let temp = TempDir::new("serialize");
        let root = temp.path();
        create_sdk(root);
        let layout = inspect_sdk(root).unwrap();
        let environment = build_environment(
            SdkCandidate {
                path: root.to_path_buf(),
                source: AndroidSdkSource::CommonLocation,
            },
            layout,
            Some(JavaRuntime {
                path: "/jdk".to_string(),
                version: Some("25.0.3".to_string()),
                source: JavaSource::AndroidStudio,
            }),
            vec!["Pixel_8".to_string()],
            Vec::new(),
        );

        let json = serde_json::to_value(&environment).unwrap();

        assert_eq!(json["status"], "ok");
        assert_eq!(json["sdkSource"], "commonLocation");
        assert_eq!(json["java"]["source"], "androidStudio");
        assert_eq!(json["acceptedLicenses"], 1);
    }
}
