use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{DepcheckError, Ecosystem};

fn collect_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            children.push(entry.path());
        }
        children.sort();
        for child in children.into_iter().rev() {
            if child.is_dir() {
                stack.push(child);
            } else if child.is_file() {
                out.push(child);
            }
        }
    }
    out.sort();
    out
}

pub fn is_test_file(eco: Ecosystem, path: &Path) -> bool {
    let s = format!("/{}", dx_path::posix(path).trim_start_matches('/'));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();
    match eco {
        Ecosystem::Rust => {
            let stem = name.strip_suffix(".rs").unwrap_or(&name);
            s.contains("/tests/")
                || s.contains("/benches/")
                || name.ends_with("_test.rs")
                || name.ends_with("_tests.rs")
                || stem == "test"
                || stem == "tests"
        }
        Ecosystem::Python => {
            name.starts_with("test_") || name.ends_with("_test.py") || s.contains("/tests/")
        }
        Ecosystem::Go => {
            name.ends_with("_test.go") || s.contains("/tests/") || s.contains("/test/")
        }
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => {
            name.contains("Test") || s.to_lowercase().contains("/test/") || s.contains("/tests/")
        }
        Ecosystem::Csharp | Ecosystem::Fsharp => {
            name.contains("Test") || s.to_lowercase().contains("/test/") || s.contains("/tests/")
        }
        Ecosystem::Cc => {
            name.to_lowercase().contains("test")
                || s.to_lowercase().contains("/test/")
                || s.contains("/tests/")
        }
        Ecosystem::Js | Ecosystem::Ts => {
            name.ends_with(".test.js")
                || name.ends_with(".test.ts")
                || s.contains("/__tests__/")
                || s.contains("/tests/")
        }
        Ecosystem::Ruby => {
            name.ends_with("_spec.rb") || name.ends_with("_test.rb") || s.contains("/tests/")
        }
    }
}

fn source_suffixes() -> BTreeSet<&'static str> {
    [
        ".rs", ".py", ".js", ".ts", ".mjs", ".cjs", ".jsx", ".tsx", ".go", ".java", ".kt", ".kts",
        ".scala", ".cs", ".fs", ".fsi", ".fsx", ".cc", ".cpp", ".cxx", ".c", ".h", ".hpp", ".rb",
    ]
    .into_iter()
    .collect()
}

fn skip_names() -> BTreeSet<&'static str> {
    [
        "Cargo.toml",
        "Cargo.lock",
        "pyproject.toml",
        "uv.lock",
        "package.json",
        "pnpm-lock.yaml",
        "go.mod",
        "go.sum",
        "jvm_deps.toml",
        "maven_install.json",
        "paket.dependencies",
        "paket.lock",
        "cc_deps.toml",
        "cc_lock.json",
        "Gemfile",
        "Gemfile.lock",
        "depcheck_exceptions.toml",
    ]
    .into_iter()
    .collect()
}

fn regex_escape(text: &str) -> String {
    regex::escape(text)
}

#[derive(Clone, Copy)]
enum FileKind {
    Src,
    Test,
    Build,
}

struct DepPatterns {
    strong: Vec<regex::Regex>,
    weak: Vec<regex::Regex>,
    shadow: Option<regex::Regex>,
}

impl DepPatterns {
    fn is_match(&self, text: &str) -> bool {
        if self.strong.iter().any(|re| re.is_match(text)) {
            return true;
        }
        if self.shadow.as_ref().is_some_and(|re| re.is_match(text)) {
            return false;
        }
        self.weak.iter().any(|re| re.is_match(text))
    }
}

fn compile(eco: Ecosystem, patterns: Vec<String>) -> Result<Vec<regex::Regex>, DepcheckError> {
    patterns
        .into_iter()
        .map(|pat| {
            if matches!(eco, Ecosystem::Csharp | Ecosystem::Fsharp) {
                regex::Regex::new(&format!("(?i){pat}"))
            } else {
                regex::Regex::new(&pat)
            }
            .map_err(DepcheckError::SourcesRegex)
        })
        .collect()
}

fn jvm_patterns(dep: &str) -> Vec<String> {
    let (group, art) = dep.rsplit_once(':').unwrap_or(("", dep));
    let art_dash = art.replace(['-', '.'], "_");
    let head = r"(?m)^\s*import\s+(?:static\s+)?";
    let prefix = match group.split_once('.') {
        Some((first, _)) => format!(r"(?:{}\.(?:\w+\.)*)?", regex_escape(first)),
        None => r"(?:\w+\.)*".to_owned(),
    };
    [art, art_dash.as_str()]
        .iter()
        .map(|name| format!(r"{head}{prefix}{}(?:\.|\b)", regex_escape(name)))
        .collect()
}

fn dep_patterns(eco: Ecosystem, dep: &str) -> Result<DepPatterns, DepcheckError> {
    let mut weak = Vec::new();
    let mut shadow = None;
    let strong: Vec<String> = match eco {
        Ecosystem::Rust => {
            let c = regex_escape(&dep.replace('-', "_"));
            weak = vec![format!(r"\buse\s+{c}\b"), format!(r"(?:^|[^\w:]){c}\s*::")];
            shadow = Some(format!(
                r"(?m)^\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:mod\s+{c}\s*[;{{]|use\s+(?:crate|self|super)(?:::\w+)*::{c}\s*;)"
            ));
            vec![
                format!(r"\bextern\s+crate\s+{c}\b"),
                format!(r"(?:^|[^\w:])::{c}\s*::"),
                format!(r"\buse\s+::{c}\b"),
            ]
        }
        Ecosystem::Python => {
            let raw_dash = dep.replace('_', "-");
            vec![
                format!(r"(?m)^\s*import\s+{}\b", regex_escape(dep)),
                format!(r"(?m)^\s*from\s+{}\b", regex_escape(dep)),
                format!(r"(?m)^\s*import\s+{}\b", regex_escape(&raw_dash)),
                format!(r"(?m)^\s*from\s+{}\b", regex_escape(&raw_dash)),
            ]
        }
        Ecosystem::Go => vec![
            format!(r#"import\s+(?:\(\s*)?["']{}["']"#, regex_escape(dep)),
            format!(r#"["']{}(?:/[^"']*)?["']"#, regex_escape(dep)),
        ],
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => jvm_patterns(dep),
        Ecosystem::Csharp | Ecosystem::Fsharp => {
            let base = dep.split('.').next_back().unwrap_or(dep).to_owned();
            vec![
                format!(r"(?m)^\s*(using|open)\s+.*{}\b", regex_escape(dep)),
                format!(r"(?m)^\s*(using|open)\s+.*{}\b", regex_escape(&base)),
            ]
        }
        Ecosystem::Cc => {
            let cname = dep.replace('-', "_");
            [dep, cname.as_str()]
                .iter()
                .map(|d| {
                    format!(
                        r#"#\s*include\s*[<"](?:[^>"]*/)?{}(?:/[^>"]*|\.(?:h|hh|hpp|hxx|inc))?[>"]"#,
                        regex_escape(d)
                    )
                })
                .collect()
        }
        Ecosystem::Js | Ecosystem::Ts => {
            let spec = format!(r#"['"]{}(?:/[^'"]*)?['"]"#, regex_escape(dep));
            vec![
                format!(r"from\s+{spec}"),
                format!(r"require\(\s*{spec}\s*\)"),
                format!(r"import\(\s*{spec}\s*\)"),
                format!(r"(?m)^\s*import\s+{spec}"),
            ]
        }
        Ecosystem::Ruby => vec![
            format!(r#"(?m)^\s*require\s+['"]{}['"]"#, regex_escape(dep)),
            format!(
                r#"(?m)^\s*require\s+['"]{}(?:/[^'"]*)?['"]"#,
                regex_escape(dep)
            ),
        ],
    };
    Ok(DepPatterns {
        strong: compile(eco, strong)?,
        weak: compile(eco, weak)?,
        shadow: compile(eco, shadow.into_iter().collect())?.pop(),
    })
}

pub fn find_usages(
    eco: Ecosystem,
    sources_root: &Path,
    dep_names: &[String],
) -> Result<BTreeMap<String, crate::Usage>, DepcheckError> {
    let mut out: BTreeMap<String, crate::Usage> = dep_names
        .iter()
        .map(|k| (k.clone(), crate::Usage::default()))
        .collect();
    let files = collect_sources(sources_root);
    let skips = skip_names();
    let suffixes = source_suffixes();
    let cfg_test =
        regex::Regex::new(r"(?m)^\s*#!\[cfg\(test\)\]").map_err(DepcheckError::SourcesRegex)?;
    let mut texts: Vec<(FileKind, String)> = Vec::new();
    for file in files {
        let Some(fname) = file.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if skips.contains(fname) {
            continue;
        }
        let has_suffix = suffixes.iter().any(|s| fname.ends_with(s));
        if !has_suffix {
            continue;
        }
        let Ok(text) = std::fs::read(&file) else {
            continue;
        };
        let text = String::from_utf8_lossy(&text).into_owned();
        let rel = file.strip_prefix(sources_root).unwrap_or(&file);
        let kind = if fname == "build.rs" {
            FileKind::Build
        } else if is_test_file(eco, rel) || (eco == Ecosystem::Rust && cfg_test.is_match(&text)) {
            FileKind::Test
        } else {
            FileKind::Src
        };
        texts.push((kind, text));
    }
    for dep in dep_names {
        let patterns = dep_patterns(eco, dep)?;
        let Some(entry) = out.get_mut(dep) else {
            continue;
        };
        for (kind, text) in &texts {
            if !patterns.is_match(text) {
                continue;
            }
            match kind {
                FileKind::Build => entry.build = true,
                FileKind::Test => entry.test = true,
                FileKind::Src => entry.src = true,
            }
        }
    }
    Ok(out)
}

pub fn cmd_usage(
    eco: Ecosystem,
    manifest: &Path,
    sources: &Path,
    exceptions: Option<&Path>,
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> i32 {
    if !manifest.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: manifest missing: {}",
            manifest.display()
        );
        return 2;
    }
    if !sources.exists() {
        let _ = writeln!(
            stderr,
            "depcheck: ERROR: sources missing: {}",
            sources.display()
        );
        return 2;
    }
    let mut deps = match crate::consistency::load_manifest(eco, manifest) {
        Ok(deps) => deps,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let exc = match crate::exceptions::parse_exceptions(exceptions) {
        Ok(exc) => exc,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    let mut norm_exc: BTreeMap<String, crate::Exception> = BTreeMap::new();
    for value in exc.values() {
        norm_exc.insert(
            crate::exceptions::normalize_exception_key(eco, &value.raw),
            value.clone(),
        );
    }
    let dep_names: Vec<String> = deps.keys().cloned().collect();
    let usages = match find_usages(eco, sources, &dep_names) {
        Ok(usages) => usages,
        Err(err) => {
            let _ = writeln!(stderr, "depcheck: ERROR: {err}");
            return 2;
        }
    };
    let mut failures = Vec::new();
    for (key, value) in &norm_exc {
        if !deps.contains_key(key) {
            failures.push(format!(
                "obsolete exception for removed dependency '{}' (report only, not deleted)",
                value.raw
            ));
        }
    }
    for (name, info) in &deps {
        let usage = usages.get(name).copied().unwrap_or_default();
        let used_any = usage.src || usage.test || usage.build;
        let has_exc = norm_exc.contains_key(name);
        let reason = norm_exc
            .get(name)
            .map(|v| v.reason.clone())
            .unwrap_or_default();
        if has_exc && reason.is_empty() {
            failures.push(format!(
                "exception for '{}' missing reason (each exception needs an explanatory reason)",
                info.raw
            ));
            continue;
        }
        if has_exc && used_any {
            failures.push(format!(
                "obsolete exception for '{}' (usage recognized; remove the exception)",
                info.raw
            ));
            continue;
        }
        if has_exc && !used_any {
            continue;
        }
        if used_any {
            if info.category == "prod" && !usage.src && !usage.build && usage.test {
                failures.push(format!(
                    "category error: production declaration '{}' used only by tests (move to dev)",
                    info.raw
                ));
                continue;
            }
            if eco == Ecosystem::Rust
                && info.category == "prod"
                && !usage.src
                && usage.build
                && !usage.test
            {
                failures.push(format!(
                    "category error: production declaration '{}' used only by build tooling (move to build-dependencies)",
                    info.raw
                ));
                continue;
            }
            continue;
        }
        if info.optional || info.platform {
            let kind = if info.optional {
                "optional"
            } else {
                "platform-specific"
            };
            failures.push(format!(
                "unused {kind} declaration '{}' (optional/platform is not proof of usage)",
                info.raw
            ));
        } else {
            failures.push(format!("unused declaration '{}'", info.raw));
        }
    }
    if !failures.is_empty() {
        for failure in &failures {
            let _ = writeln!(stderr, "depcheck: FAIL: usage: {failure}");
        }
        return 1;
    }
    let _ = writeln!(
        stdout,
        "depcheck: OK: usage: {} declarations used in owning scope ({})",
        deps.len(),
        eco.name()
    );
    0
}
