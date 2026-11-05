//! Reads and rewrites a package's own version in the manifests a workspace has.
//! Edits are text edits: only the bytes of the version string change.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::{fs, io};

use super::request::plans::JsonScan;

const SKIPPED_DIRS: [&str; 4] = ["node_modules", "target", "third_party", ".git"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Cargo,
    PackageJson,
    Pyproject,
    Pubspec,
    Gradle,
    Csproj,
    Mix,
    ModuleBazel,
    Gemspec,
    Chart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub path: PathBuf,
    pub kind: Kind,
    pub version: String,
}

impl Kind {
    fn of(file_name: &str) -> Option<Kind> {
        Some(match file_name {
            "Cargo.toml" => Kind::Cargo,
            "package.json" => Kind::PackageJson,
            "pyproject.toml" => Kind::Pyproject,
            "pubspec.yaml" => Kind::Pubspec,
            "build.gradle" | "build.gradle.kts" => Kind::Gradle,
            "mix.exs" => Kind::Mix,
            "MODULE.bazel" => Kind::ModuleBazel,
            "Chart.yaml" => Kind::Chart,
            name if name.ends_with(".csproj") => Kind::Csproj,
            name if name.ends_with(".gemspec") => Kind::Gemspec,
            _ => return None,
        })
    }

    /// Byte ranges of the version text, without quotes, in file order.
    pub fn spans(self, content: &str) -> Vec<Range<usize>> {
        match self {
            Kind::Cargo => toml_spans(content, &["package", "workspace.package"]),
            Kind::Pyproject => toml_spans(content, &["project", "tool.poetry"]),
            Kind::PackageJson => json_span(content).into_iter().collect(),
            Kind::Pubspec | Kind::Chart => line_spans(content, yaml_value),
            Kind::Gradle => line_spans(content, gradle_value),
            Kind::Mix => line_spans(content, mix_value),
            Kind::Gemspec => line_spans(content, gemspec_value),
            Kind::Csproj => csproj_span(content).into_iter().collect(),
            Kind::ModuleBazel => module_span(content).into_iter().collect(),
        }
    }

    /// The content with every version replaced, or `None` if there is none.
    pub fn rewrite(self, content: &str, version: &str) -> Option<String> {
        let spans = self.spans(content);
        if spans.is_empty() {
            return None;
        }
        let mut out = content.to_owned();
        for span in spans.into_iter().rev() {
            out.replace_range(span, version);
        }
        Some(out)
    }
}

/// Every manifest under `workspace` that carries a version, sorted by path.
pub fn find(workspace: &Path) -> io::Result<Vec<Manifest>> {
    let mut found = Vec::new();
    walk(workspace, &mut found)?;
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// Writes `version` into the manifest file.
pub fn set(manifest: &Manifest, version: &str) -> io::Result<()> {
    let content = fs::read_to_string(&manifest.path)?;
    let Some(updated) = manifest.kind.rewrite(&content, version) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} has no version", manifest.path.display()),
        ));
    };
    fs::write(&manifest.path, updated)
}

fn walk(dir: &Path, found: &mut Vec<Manifest>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !SKIPPED_DIRS.contains(&name.as_str()) && !name.starts_with("bazel-") {
                walk(&entry.path(), found)?;
            }
        } else if file_type.is_file() {
            let Some(kind) = Kind::of(&name) else {
                continue;
            };
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };
            if let Some(span) = kind.spans(&content).into_iter().next() {
                found.push(Manifest {
                    path: entry.path(),
                    kind,
                    version: content[span].to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut at = 0;
    content.split_inclusive('\n').map(move |line| {
        let start = at;
        at += line.len();
        (start, line)
    })
}

/// The text between the first pair of matching quotes, as offsets into `text`.
fn quoted(text: &str) -> Option<Range<usize>> {
    let quote = text.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let end = text[1..].find(quote)?;
    Some(1..1 + end)
}

fn shifted(base: usize, line: &str, rest: &str, span: Range<usize>) -> Range<usize> {
    let offset = base + line.len() - rest.len();
    offset + span.start..offset + span.end
}

fn line_spans(
    content: &str,
    value: impl Fn(&str) -> Option<(&str, Range<usize>)>,
) -> Vec<Range<usize>> {
    for (start, line) in lines(content) {
        if let Some((rest, span)) = value(line) {
            return vec![shifted(start, line, rest, span)];
        }
    }
    Vec::new()
}

fn toml_spans(content: &str, tables: &[&str]) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut inside = false;
    for (start, line) in lines(content) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            let name = trimmed.trim_start_matches('[');
            let name = name.split(']').next().unwrap_or("").trim();
            inside = !trimmed.starts_with("[[") && tables.contains(&name);
        } else if inside {
            if let Some(rest) = key_value(line.trim_start(), "version", '=') {
                if let Some(span) = quoted(rest) {
                    spans.push(shifted(start, line, rest, span));
                }
            }
        }
    }
    spans
}

/// The text after `key`, optional spaces, `sep` and optional spaces.
fn key_value<'a>(text: &'a str, key: &str, sep: char) -> Option<&'a str> {
    let rest = text.strip_prefix(key)?.trim_start();
    Some(rest.strip_prefix(sep)?.trim_start())
}

fn yaml_value(line: &str) -> Option<(&str, Range<usize>)> {
    let rest = key_value(line, "version", ':')?;
    if let Some(span) = quoted(rest) {
        return Some((rest, span));
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '#')
        .unwrap_or(rest.len());
    (end > 0).then_some((rest, 0..end))
}

fn gradle_value(line: &str) -> Option<(&str, Range<usize>)> {
    let rest = line.strip_prefix("version")?;
    if !rest.starts_with(|c: char| c == '=' || c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    Some((rest, quoted(rest)?))
}

fn mix_value(line: &str) -> Option<(&str, Range<usize>)> {
    let mut from = 0;
    while let Some(found) = line[from..].find("version:") {
        let at = from + found;
        from = at + 1;
        let before = line[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let rest = line[at + "version:".len()..].trim_start();
        if let Some(span) = quoted(rest) {
            return Some((rest, span));
        }
    }
    None
}

fn gemspec_value(line: &str) -> Option<(&str, Range<usize>)> {
    let at = line.find(".version")?;
    let rest = line[at + ".version".len()..].trim_start();
    let rest = rest.strip_prefix('=')?;
    if rest.starts_with('=') {
        return None;
    }
    let rest = rest.trim_start();
    Some((rest, quoted(rest)?))
}

fn csproj_span(content: &str) -> Option<Range<usize>> {
    let start = content.find("<Version>")? + "<Version>".len();
    let end = start + content[start..].find("</Version>")?;
    Some(start..end)
}

fn json_span(content: &str) -> Option<Range<usize>> {
    let mut scan = JsonScan {
        text: content,
        at: 0,
    };
    scan.skip_ws();
    let members = scan.members().ok()?;
    let (_, at) = members.into_iter().find(|(key, _)| key == "version")?;
    if !content[at..].starts_with('"') {
        return None;
    }
    scan.at = at;
    let end = scan.string_end().ok()?;
    Some(at + 1..end - 1)
}

fn module_span(content: &str) -> Option<Range<usize>> {
    let file = dx_module_bazel::ModuleFile::parse(content).ok()?;
    let call = file.calls_named("module").next()?;
    match call.arg("version")? {
        dx_module_bazel::Value::Str(literal) => Some(literal.span.start + 1..literal.span.end - 1),
        _ => None,
    }
}

#[cfg(test)]
#[path = "own_version_tests.rs"]
mod tests;
