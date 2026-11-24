//! pubspec.yaml / .metadata 的最小 YAML 子集解析。
//!
//! pubspec 是 YAML，但项目只关心固定字段。引入完整 YAML 解析器会带来额外依赖
//! 和边缘语义，这里实现一个只覆盖 pubspec 实际结构的缩进解析器：
//!
//! - 块映射（`key: value` / `key:` + 缩进子块）
//! - 块序列（`- item`、`- key: value` + 同缩进的后续键）
//! - 单引号 / 双引号、行尾注释、`|` / `>` 块标量
//!
//! 不支持锚点、别名、多文档和流式集合；这些结构在 pubspec 中几乎不会出现，
//! 即使出现也只是被当成普通标量，不会让整个分析失败。

use std::collections::BTreeMap;

use crate::models::project::{DependencyKind, DependencySource, ProjectDependency, ProjectMetadata};

/// YAML 子集解析后的节点。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Null,
    Scalar(String),
    List(Vec<Node>),
    Map(BTreeMap<String, Node>),
}

impl Node {
    /// 只有当标量非空时才视为有值，避免把 `key:` 当成空字符串展示。
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Scalar(value) if !value.is_empty() => Some(value),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Self::Map(map) => map.get(key),
            _ => None,
        }
    }

    fn as_list(&self) -> &[Node] {
        match self {
            Self::List(items) => items,
            _ => &[],
        }
    }
}

/// 从 pubspec.yaml 中提取的字段。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Pubspec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub publish_to: Option<String>,
    pub dart_sdk: Option<String>,
    pub flutter_sdk: Option<String>,
    pub dependencies: Vec<ProjectDependency>,
    pub dev_dependencies: Vec<ProjectDependency>,
    pub dependency_overrides: Vec<ProjectDependency>,
    pub uses_material_design: bool,
    pub assets: Vec<String>,
    pub font_families: Vec<String>,
}

/// 解析 pubspec.yaml。解析器容错：字段缺失或结构不认识时返回 None / 空列表。
pub(super) fn parse_pubspec(content: &str) -> Pubspec {
    let root = parse_document(content);
    let environment = root.get("environment");
    let flutter = root.get("flutter");

    Pubspec {
        name: text(root.get("name")),
        description: text(root.get("description")),
        version: text(root.get("version")),
        publish_to: text(root.get("publish_to")),
        dart_sdk: text(environment.and_then(|node| node.get("sdk"))),
        flutter_sdk: text(environment.and_then(|node| node.get("flutter"))),
        dependencies: collect_dependencies(root.get("dependencies"), DependencyKind::Regular),
        dev_dependencies: collect_dependencies(root.get("dev_dependencies"), DependencyKind::Dev),
        dependency_overrides: collect_dependencies(root.get("dependency_overrides"), DependencyKind::Override),
        uses_material_design: flutter
            .and_then(|node| node.get("uses-material-design"))
            .and_then(Node::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case("true")),
        assets: flutter
            .and_then(|node| node.get("assets"))
            .map(collect_assets)
            .unwrap_or_default(),
        font_families: flutter
            .and_then(|node| node.get("fonts"))
            .map(collect_font_families)
            .unwrap_or_default(),
    }
}

/// 解析 `.metadata`。Flutter 在创建项目时写入这些字段。
pub(super) fn parse_metadata(content: &str) -> ProjectMetadata {
    let root = parse_document(content);

    // `.metadata` 把 channel / revision 放在 `version:` 下面；
    // 少数工具生成的扁平结构也能兼容。
    let version = root.get("version");
    ProjectMetadata {
        channel: text(version.and_then(|node| node.get("channel"))).or_else(|| text(root.get("channel"))),
        revision: text(version.and_then(|node| node.get("revision"))).or_else(|| text(root.get("revision"))),
        project_type: text(root.get("project_type")),
    }
}

fn text(node: Option<&Node>) -> Option<String> {
    node.and_then(Node::as_str).map(str::to_string)
}

fn collect_dependencies(node: Option<&Node>, kind: DependencyKind) -> Vec<ProjectDependency> {
    let Some(Node::Map(map)) = node else {
        return Vec::new();
    };

    map.iter().map(|(name, value)| dependency(name, value, kind)).collect()
}

fn dependency(name: &str, value: &Node, kind: DependencyKind) -> ProjectDependency {
    let (constraint, source) = match value {
        Node::Scalar(text) if !text.is_empty() => (Some(text.clone()), DependencySource::Hosted),
        Node::Map(map) => {
            if let Some(sdk) = map.get("sdk").and_then(Node::as_str) {
                (Some(format!("sdk: {sdk}")), DependencySource::Sdk)
            } else if let Some(path) = map.get("path").and_then(Node::as_str) {
                (Some(format!("path: {path}")), DependencySource::Path)
            } else if let Some(git) = map.get("git") {
                // git 依赖可以写成 `git: url`，也可以写成 `git: { url: ..., ref: ... }`。
                let url = git.as_str().map(str::to_string).or_else(|| text(git.get("url")));
                (url.map(|url| format!("git: {url}")), DependencySource::Git)
            } else if let Some(version) = map.get("version").and_then(Node::as_str) {
                (Some(version.to_string()), DependencySource::Hosted)
            } else {
                (None, DependencySource::Unknown)
            }
        }
        _ => (None, DependencySource::Unknown),
    };

    ProjectDependency {
        name: name.to_string(),
        constraint,
        kind,
        source,
    }
}

fn collect_assets(node: &Node) -> Vec<String> {
    node.as_list()
        .iter()
        .filter_map(|item| match item {
            Node::Scalar(path) if !path.is_empty() => Some(path.clone()),
            Node::Map(map) => map.get("path").and_then(Node::as_str).map(str::to_string),
            Node::Null | Node::Scalar(_) | Node::List(_) => None,
        })
        .collect()
}

fn collect_font_families(node: &Node) -> Vec<String> {
    node.as_list()
        .iter()
        .filter_map(|item| text(item.get("family")))
        .filter(|family| !family.is_empty())
        .collect()
}

// ---------- YAML 子集解析 ----------

/// 预处理后的非空行：缩进宽度（字符数）与去掉缩进的内容。
#[derive(Debug, Clone)]
struct SourceLine<'a> {
    indent: usize,
    content: &'a str,
}

fn parse_document(content: &str) -> Node {
    let lines = preprocess(content);
    if lines.is_empty() {
        return Node::Map(BTreeMap::new());
    }

    let mut index = 0;
    parse_node(&lines, &mut index, lines[0].indent)
}

fn preprocess(content: &str) -> Vec<SourceLine<'_>> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = Vec::new();

    for raw in content.lines() {
        let without_comment = strip_comment(raw);
        let trimmed_end = without_comment.trim_end();

        let trimmed = trimmed_end.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed == "---" || trimmed == "..." {
            continue;
        }

        // 缩进宽度按字符计算；YAML 不允许 tab 缩进，这里仍容忍它，避免
        // 因为编辑器配置导致整份文件被跳过。
        let indent = trimmed_end.chars().take_while(|ch| *ch == ' ' || *ch == '\t').count();

        lines.push(SourceLine {
            indent,
            content: trimmed,
        });
    }

    lines
}

/// 去掉行尾注释，但保留引号内的 `#`。
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for (index, ch) in line.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && active == '"' {
                escaped = true;
                continue;
            }
            if ch == active {
                quote = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            '#' if index == 0 || line[..index].chars().next_back().is_some_and(char::is_whitespace) => {
                return &line[..index];
            }
            _ => {}
        }
    }

    line
}

fn is_sequence_item(content: &str) -> bool {
    content == "-" || content.starts_with("- ")
}

fn parse_node(lines: &[SourceLine<'_>], index: &mut usize, indent: usize) -> Node {
    if is_sequence_item(lines[*index].content) {
        parse_sequence(lines, index, indent)
    } else {
        parse_mapping(lines, index, indent)
    }
}

fn parse_mapping(lines: &[SourceLine<'_>], index: &mut usize, indent: usize) -> Node {
    let mut map = BTreeMap::new();

    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        // 缩进比当前层更深的行说明结构异常（例如缺少父键），跳过而不是死循环。
        if line.indent > indent {
            *index += 1;
            continue;
        }
        if is_sequence_item(line.content) {
            break;
        }

        let Some((key, rest)) = split_key(line.content) else {
            *index += 1;
            continue;
        };

        *index += 1;
        let value = parse_value(lines, index, indent, rest);
        map.insert(key.to_string(), value);
    }

    Node::Map(map)
}

fn parse_sequence(lines: &[SourceLine<'_>], index: &mut usize, indent: usize) -> Node {
    let mut items = Vec::new();

    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            *index += 1;
            continue;
        }
        if !is_sequence_item(line.content) {
            break;
        }

        let after_dash = &line.content[1..];
        let offset = 1 + (after_dash.len() - after_dash.trim_start().len());
        let rest = after_dash.trim_start();
        *index += 1;

        if rest.is_empty() {
            if *index < lines.len() && lines[*index].indent > indent {
                let child_indent = lines[*index].indent;
                items.push(parse_node(lines, index, child_indent));
            } else {
                items.push(Node::Null);
            }
            continue;
        }

        if let Some((key, value)) = split_key(rest) {
            // `- key: value` 的内联映射：后续同缩进的键都属于同一个列表元素。
            let child_indent = indent + offset;
            let mut map = BTreeMap::new();
            map.insert(key.to_string(), parse_value(lines, index, child_indent, value));

            while *index < lines.len()
                && lines[*index].indent == child_indent
                && !is_sequence_item(lines[*index].content)
            {
                let Some((nested_key, nested_rest)) = split_key(lines[*index].content) else {
                    break;
                };
                *index += 1;
                map.insert(nested_key.to_string(), parse_value(lines, index, child_indent, nested_rest));
            }

            items.push(Node::Map(map));
        } else {
            items.push(Node::Scalar(parse_scalar(rest)));
            // 列表标量后面不应再有更深的内容；跳过以防格式异常时反复解析。
            while *index < lines.len() && lines[*index].indent > indent {
                *index += 1;
            }
        }
    }

    Node::List(items)
}

fn parse_value(lines: &[SourceLine<'_>], index: &mut usize, parent_indent: usize, rest: &str) -> Node {
    if rest.is_empty() {
        if *index < lines.len() && lines[*index].indent > parent_indent {
            let child_indent = lines[*index].indent;
            return parse_node(lines, index, child_indent);
        }
        return Node::Null;
    }

    // 块标量：`|` 保留换行，`>` 折叠成空格。只取内容文本，忽略缩进层级。
    if rest.starts_with('|') || rest.starts_with('>') {
        let folded = rest.starts_with('>');
        let mut parts = Vec::new();
        while *index < lines.len() && lines[*index].indent > parent_indent {
            parts.push(lines[*index].content.trim().to_string());
            *index += 1;
        }
        let value = if folded { parts.join(" ") } else { parts.join("\n") };
        return Node::Scalar(value);
    }

    Node::Scalar(parse_scalar(rest))
}

/// 在行内找到第一个「键: 值」分隔符。冒号后面必须是空白或行尾，
/// 这样 `path: ../foo:bar` 之类的值不会被误判。
fn split_key(content: &str) -> Option<(&str, &str)> {
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for (index, ch) in content.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && active == '"' {
                escaped = true;
                continue;
            }
            if ch == active {
                quote = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            ':' => {
                let rest = &content[index + 1..];
                if rest.is_empty() || rest.starts_with([' ', '\t']) {
                    let key = content[..index].trim().trim_matches(['\'', '"']);
                    if key.is_empty() {
                        return None;
                    }
                    return Some((key, rest.trim_start()));
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_scalar(value: &str) -> String {
    let value = value.trim();

    if let Some(inner) = value.strip_prefix('\'').and_then(|text| text.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    if let Some(inner) = value.strip_prefix('"').and_then(|text| text.strip_suffix('"')) {
        return unescape_double_quoted(inner);
    }

    value.to_string()
}

fn unescape_double_quoted(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBSPEC: &str = r#"
name: demo_app
description: "A demo # not a comment"
publish_to: 'none'
version: 1.2.3+4

environment:
  sdk: ^3.5.0
  flutter: ">=3.24.0"

dependencies:
  flutter:
    sdk: flutter
  http: ^1.2.0
  local_util:
    path: ../local_util
  git_pkg:
    git:
      url: https://github.com/example/git_pkg.git
      ref: main
  any_pkg: any

dev_dependencies:
  flutter_test:
    sdk: flutter
  flutter_lints: ^4.0.0

dependency_overrides:
  meta: 1.16.0

flutter:
  uses-material-design: true
  assets:
    - assets/logo.png
    - assets/icons/
  fonts:
    - family: Demo
      fonts:
        - asset: fonts/Demo-Regular.ttf
    - family: Mono
      fonts:
        - asset: fonts/Mono.ttf
"#;

    fn dependency<'a>(items: &'a [ProjectDependency], name: &str) -> &'a ProjectDependency {
        items
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("缺少依赖 {name}"))
    }

    #[test]
    fn parses_realistic_pubspec() {
        let pubspec = parse_pubspec(PUBSPEC);

        assert_eq!(pubspec.name.as_deref(), Some("demo_app"));
        assert_eq!(pubspec.description.as_deref(), Some("A demo # not a comment"));
        assert_eq!(pubspec.publish_to.as_deref(), Some("none"));
        assert_eq!(pubspec.version.as_deref(), Some("1.2.3+4"));
        assert_eq!(pubspec.dart_sdk.as_deref(), Some("^3.5.0"));
        assert_eq!(pubspec.flutter_sdk.as_deref(), Some(">=3.24.0"));

        assert_eq!(pubspec.dependencies.len(), 5);
        assert_eq!(dependency(&pubspec.dependencies, "http").constraint.as_deref(), Some("^1.2.0"));
        assert_eq!(dependency(&pubspec.dependencies, "flutter").source, DependencySource::Sdk);
        assert_eq!(dependency(&pubspec.dependencies, "local_util").source, DependencySource::Path);
        assert_eq!(dependency(&pubspec.dependencies, "git_pkg").source, DependencySource::Git);
        assert_eq!(
            dependency(&pubspec.dependencies, "git_pkg").constraint.as_deref(),
            Some("git: https://github.com/example/git_pkg.git")
        );

        assert_eq!(pubspec.dev_dependencies.len(), 2);
        assert_eq!(dependency(&pubspec.dev_dependencies, "flutter_test").source, DependencySource::Sdk);
        assert_eq!(pubspec.dependency_overrides.len(), 1);
        assert_eq!(dependency(&pubspec.dependency_overrides, "meta").kind, DependencyKind::Override);

        assert!(pubspec.uses_material_design);
        assert_eq!(pubspec.assets, vec!["assets/logo.png", "assets/icons/"]);
        assert_eq!(pubspec.font_families, vec!["Demo", "Mono"]);
    }

    #[test]
    fn parses_block_scalars_and_comments() {
        let pubspec = parse_pubspec(
            "name: block_app # 行尾注释\n\
             description: |\n\
             \x20 first line\n\
             \x20 second line\n\
             flutter:\n\
             \x20 uses-material-design: false\n",
        );

        assert_eq!(pubspec.name.as_deref(), Some("block_app"));
        assert_eq!(pubspec.description.as_deref(), Some("first line\nsecond line"));
        assert!(!pubspec.uses_material_design);
    }

    #[test]
    fn parses_metadata() {
        let metadata = parse_metadata(
            "version:\n\
             \x20 revision: abc123\n\
             \x20 channel: stable\n\
             project_type: app\n",
        );

        assert_eq!(metadata.channel.as_deref(), Some("stable"));
        assert_eq!(metadata.revision.as_deref(), Some("abc123"));
        assert_eq!(metadata.project_type.as_deref(), Some("app"));
    }

    #[test]
    fn tolerates_missing_and_malformed_sections() {
        let pubspec = parse_pubspec("");

        assert_eq!(pubspec.name, None);
        assert!(pubspec.dependencies.is_empty());
        assert!(pubspec.assets.is_empty());

        let broken = parse_pubspec("name: broken\n  stray: value\ndependencies: not-a-map\n");
        assert_eq!(broken.name.as_deref(), Some("broken"));
        assert!(broken.dependencies.is_empty());
    }
}
