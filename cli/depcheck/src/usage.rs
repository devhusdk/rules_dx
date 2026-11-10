use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dx_error::DxError;

use crate::consistency::reported;
use crate::{DepcheckError, Ecosystem};

fn collect_sources(root: &Path, nested: Option<&str>) -> Vec<PathBuf> {
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
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if child.is_dir() {
                if !matches!(name, ".git" | "node_modules" | "target" | ".dart_tool")
                    && !name.starts_with("bazel-")
                    && !nested.is_some_and(|file| child.join(file).is_file())
                {
                    stack.push(child);
                }
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
        Ecosystem::Dart => {
            name.ends_with("_test.dart") || s.contains("/test/") || s.contains("/integration_test/")
        }
        Ecosystem::Ruby => {
            name.ends_with("_spec.rb") || name.ends_with("_test.rb") || s.contains("/tests/")
        }
        Ecosystem::Julia => name == "runtests.jl" || s.contains("/test/") || s.contains("/tests/"),
        Ecosystem::Zig => {
            name.ends_with("_test.zig") || s.contains("/test/") || s.contains("/tests/")
        }
        Ecosystem::Php => {
            name.ends_with("Test.php") || s.contains("/tests/") || s.contains("/Tests/")
        }
        Ecosystem::Fortran | Ecosystem::D => {
            s.contains("/test/") || s.contains("/tests/") || name.contains("_test.")
        }
        Ecosystem::Haskell => {
            name.ends_with("Spec.hs")
                || name.ends_with("Test.hs")
                || name.ends_with("Tests.hs")
                || s.contains("/test/")
                || s.contains("/tests/")
        }
        Ecosystem::Swift => s.contains("/Tests/") || name.ends_with("Tests.swift"),
        Ecosystem::Powershell => name.ends_with(".Tests.ps1") || s.contains("/tests/"),
        Ecosystem::Lua => {
            name.ends_with("_spec.lua") || s.contains("/spec/") || s.contains("/tests/")
        }
        Ecosystem::Ocaml => s.contains("/test/") || s.contains("/tests/"),
        Ecosystem::Perl => name.ends_with(".t") || s.contains("/t/") || s.contains("/xt/"),
        Ecosystem::Elixir => name.ends_with("_test.exs") || s.contains("/test/"),
        Ecosystem::Erlang => name.ends_with("_SUITE.erl") || s.contains("/test/"),
    }
}

fn source_suffixes() -> BTreeSet<&'static str> {
    [
        ".rs",
        ".py",
        ".js",
        ".ts",
        ".mjs",
        ".cjs",
        ".jsx",
        ".tsx",
        ".go",
        ".java",
        ".kt",
        ".kts",
        ".scala",
        ".cs",
        ".fs",
        ".fsi",
        ".fsx",
        ".cc",
        ".cpp",
        ".cxx",
        ".c",
        ".h",
        ".hpp",
        ".rb",
        ".pl",
        ".pm",
        ".t",
        ".psgi",
        ".zig",
        ".jl",
        ".dart",
        ".php",
        "analysis_options.yaml",
        ".ex",
        ".exs",
        ".erl",
        ".hrl",
        ".f90",
        ".f95",
        ".f03",
        ".f08",
        ".F90",
        ".f",
        ".d",
        ".di",
        ".hs",
        ".lhs",
        ".swift",
        ".ps1",
        ".psm1",
        ".lua",
        "dune",
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
        "cpanfile",
        "cpanfile.snapshot",
        "build.zig.zon",
        "build.zig.zon.lock",
        "Project.toml",
        "JuliaProject.toml",
        "Manifest.toml",
        "JuliaManifest.toml",
        "pubspec.yaml",
        "pubspec.lock",
        "composer.json",
        "composer.lock",
        "mix.exs",
        "mix.lock",
        "rebar.lock",
        "fpm.toml",
        "fpm.lock",
        "dub.json",
        "dub.sdl",
        "dub.selections.json",
        "cabal.project.freeze",
        "stack.yaml.lock",
        "Package.swift",
        "Package.resolved",
        "luarocks.lock",
        "opam.locked",
    ]
    .into_iter()
    .collect()
}

/// Rocks whose module name is unrelated to the rock name.
fn lua_module_alias(rock: &str) -> Option<&'static str> {
    match rock {
        "penlight" => Some("pl"),
        "luafilesystem" => Some("lfs"),
        "luaposix" => Some("posix"),
        _ => None,
    }
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

impl DepPatterns {
    /// Byte offset of the first import, honouring the same shadow rule as `is_match`.
    fn first_match(&self, text: &str) -> Option<usize> {
        let first = |patterns: &[regex::Regex]| {
            patterns
                .iter()
                .filter_map(|re| re.find(text))
                .map(|found| {
                    let lead = found.as_str().len() - found.as_str().trim_start().len();
                    found.start() + lead
                })
                .min()
        };
        if let Some(at) = first(&self.strong) {
            return Some(at);
        }
        if self.shadow.as_ref().is_some_and(|re| re.is_match(text)) {
            return None;
        }
        first(&self.weak)
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

/// Composer names are `vendor/package`; namespaces follow the vendor (`GuzzleHttp\\`) or put the
/// package below it (`Symfony\\Component\\Console`). Matching is case-insensitive.
fn php_patterns(dep: &str) -> Vec<String> {
    let (vendor, package) = dep.split_once('/').unwrap_or((dep, dep));
    let separators = |name: &str| {
        name.split(['-', '_', '.'])
            .map(regex_escape)
            .collect::<Vec<_>>()
            .join("[-_.]?")
    };
    let vendor_ns = separators(vendor);
    let package_ns = package
        .split(['-', '_', '.'])
        .map(regex_escape)
        .collect::<String>();
    let head = r"(?mi)(?:^\s*use\s+(?:function\s+|const\s+)?\\?|[\s(,=!]\\)";
    let mut patterns = vec![format!(r"{head}{vendor_ns}\\(?:\w+\\)*{package_ns}\b")];
    let near = vendor == package || vendor.starts_with(package) || package.starts_with(vendor);
    if near {
        patterns.push(format!(r"{head}{vendor_ns}\\"));
    }
    patterns
}

/// The modules a dependency is called by: `ex_doc` is `ExDoc`, and `plug_cowboy` is also `Plug`
/// because its modules live under `Plug.Cowboy`.
fn elixir_modules(dep: &str) -> Vec<String> {
    let camel = |part: &str| {
        let mut chars = part.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect::<String>())
            .unwrap_or_default()
    };
    let mut modules = vec![dep.split('_').map(camel).collect::<String>()];
    let head = camel(dep.split('_').next().unwrap_or(dep));
    if !modules.contains(&head) {
        modules.push(head);
    }
    modules
}

/// Extra spellings a source can use for a package: a dub package `vibe-d` is imported as `vibe`.
fn module_forms(eco: Ecosystem, dep: &str) -> Vec<String> {
    if eco != Ecosystem::D {
        return Vec::new();
    }
    let root = dep.replace('-', "_");
    let mut forms = vec![root.clone()];
    if let Some(bare) = root.strip_suffix("_d").filter(|bare| !bare.is_empty()) {
        forms.push(bare.to_owned());
    }
    forms
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
        Ecosystem::Powershell => {
            let module = regex_escape(dep);
            let end = r#"(?:['"\s,;)]|$)"#;
            vec![
                format!(r#"(?mi)^\s*Import-Module\s+(?:-Name\s+)?['"]?{module}{end}"#),
                format!(r#"(?mi)^\s*using\s+module\s+['"]?{module}{end}"#),
                format!(
                    r#"(?mi)^\s*#requires\s+.*-modules\s+(?:[\w.-]+\s*,\s*)*['"]?{module}{end}"#
                ),
            ]
        }
        Ecosystem::Julia => {
            let name = regex_escape(dep);
            vec![format!(
                r"(?m)^\s*(?:using|import)\s+(?:[^\n#:]*,\s*)?{name}\b(?:\s*[:,]|\s*(?:#.*)?$|\.)"
            )]
        }
        Ecosystem::Lua => {
            let mut names = vec![dep.to_owned()];
            let trimmed = dep.strip_suffix(".lua").unwrap_or(dep);
            let trimmed = trimmed.strip_prefix("lua-").unwrap_or(trimmed);
            names.push(trimmed.to_owned());
            names.push(trimmed.replace('-', "_"));
            names.push(trimmed.strip_prefix("lua").unwrap_or(trimmed).to_owned());
            names.extend(lua_module_alias(dep).map(str::to_owned));
            names.sort();
            names.dedup();
            let names: Vec<String> = names.iter().map(|name| regex_escape(name)).collect();
            vec![format!(
                r#"\brequire\s*\(?\s*['"](?:{})(?:\.[^'"]*)?['"]"#,
                names.join("|")
            )]
        }
        Ecosystem::Ocaml => {
            let name = regex_escape(dep);
            vec![format!(
                r"\((?:libraries|pps|preprocess_deps)\b[^()]*?[\s(]{name}(?:\.[\w.]+)?[\s)]"
            )]
        }
        Ecosystem::Zig => {
            let name = regex_escape(dep);
            vec![
                format!(r#"\b(?:lazyDependency|dependency)\(\s*"{name}""#),
                format!(r#"@import\(\s*"{name}"\s*\)"#),
            ]
        }
        Ecosystem::Swift => {
            let module = regex_escape(dep);
            vec![format!(
                r"(?m)^\s*(?:@\w+(?:\([^)]*\))?\s+)*import\s+(?:(?:struct|class|enum|protocol|typealias|func|var|let)\s+)?{module}\b"
            )]
        }
        Ecosystem::Perl => {
            let module = regex_escape(dep);
            vec![
                format!(r"(?m)^\s*(?:use|require|no)\s+{module}\b"),
                format!(
                    r#"(?m)^\s*use\s+(?:parent|base)\s+(?:-norequire\s*,\s*)?(?:qw\s*[(/\[{{]\s*(?:[\w:]+\s+)*|['"](?:[\w:]+['"]\s*,\s*['"])*){module}\b"#
                ),
            ]
        }
        Ecosystem::Dart => {
            let dep = regex_escape(dep);
            vec![
                format!(r#"(?m)^\s*(?:import|export|part)\s+['"]package:{dep}/"#),
                format!(r#"(?m)^\s*include:\s*['"]?package:{dep}/"#),
            ]
        }
        Ecosystem::Php => php_patterns(dep),
        Ecosystem::Elixir => {
            let mut patterns = vec![format!(r"(?:^|[^\w:]):{}\b", regex_escape(dep))];
            for module in elixir_modules(dep) {
                let module = regex_escape(&module);
                patterns.push(format!(
                    r"(?m)^\s*(?:alias|import|use|require)\s+{module}\b"
                ));
                patterns.push(format!(r"\b{module}\.\w"));
            }
            patterns
        }
        Ecosystem::Erlang => vec![
            format!(r#"(?m)^\s*-include(?:_lib)?\(\s*"{}/"#, regex_escape(dep)),
            format!(r"(?m)^\s*-behaviou?r\(\s*{}\s*\)", regex_escape(dep)),
            format!(r"(?:^|[^\w:]){}:[a-z_]\w*\(", regex_escape(dep)),
        ],
        Ecosystem::Fortran => {
            let module = regex_escape(&dep.replace('-', "_"));
            vec![format!(
                r"(?mi)^\s*use\s*(?:,\s*(?:non_)?intrinsic\s*)?(?:::\s*)?{module}(?:_\w+)?\b"
            )]
        }
        Ecosystem::D => {
            let roots: Vec<String> = module_forms(eco, dep)
                .iter()
                .map(|root| regex_escape(root))
                .collect();
            vec![format!(
                r"(?m)^\s*(?:(?:public|private|static)\s+)*import\s+(?:[^;]*?[,=]\s*)?(?:{})(?:\.|\s*[;,:])",
                roots.join("|")
            )]
        }
        Ecosystem::Haskell => {
            let head = r#"(?m)^\s*import\s+(?:safe\s+)?(?:qualified\s+)?"#;
            let package = format!(r#"(?:"{}"\s+(?:qualified\s+)?)?"#, regex_escape(dep));
            crate::ecosystem::haskell::module_prefixes(dep)
                .iter()
                .map(|module| format!(r"{head}(?:{package}){}(?:[.\s(]|$)", regex_escape(module)))
                .chain([format!(r#"{head}"{}"\s+[A-Z]"#, regex_escape(dep))])
                .collect()
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

/// First `file:line` under `root` that imports each of `names`. Directories holding their own
/// `nested` manifest are not entered.
pub(crate) fn find_first_imports(
    eco: Ecosystem,
    root: &Path,
    nested: &str,
    names: &[String],
) -> Result<BTreeMap<String, (String, usize)>, DepcheckError> {
    let skips = skip_names();
    let suffixes = source_suffixes();
    let mut texts: Vec<(String, String)> = Vec::new();
    for file in collect_sources(root, Some(nested)) {
        let Some(fname) = file.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if skips.contains(fname) || !suffixes.iter().any(|s| fname.ends_with(s)) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let rel = file.strip_prefix(root).unwrap_or(&file);
        texts.push((
            dx_path::posix(rel),
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
    }
    let mut out = BTreeMap::new();
    for name in names {
        let mut forms = vec![name.clone(), name.replace('-', "_"), name.replace('_', "-")];
        forms.extend(elixir_modules(name));
        forms.extend(module_forms(eco, name));
        if eco == Ecosystem::Haskell {
            forms.extend(crate::ecosystem::haskell::module_prefixes(name));
        }
        let hits: Vec<&(String, String)> = texts
            .iter()
            .filter(|(_, text)| {
                if eco == Ecosystem::Php {
                    return true;
                }
                let text = if eco == Ecosystem::Powershell {
                    text.to_lowercase()
                } else {
                    text.clone()
                };
                forms.iter().any(|form| text.contains(form.as_str()))
            })
            .collect();
        if hits.is_empty() {
            continue;
        }
        let patterns = dep_patterns(eco, name)?;
        for (rel, text) in hits {
            if let Some(at) = patterns.first_match(text) {
                let line = text[..at].matches('\n').count() + 1;
                out.insert(name.clone(), (rel.clone(), line));
                break;
            }
        }
    }
    Ok(out)
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
    let files = collect_sources(sources_root, None);
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
    stdout: &mut dyn std::fmt::Write,
    stderr: &mut dyn std::fmt::Write,
) -> Result<(), DxError> {
    if !manifest.exists() {
        return Err(reported(
            stderr,
            DxError::usage(format!("manifest missing: {}", manifest.display())),
        ));
    }
    if !sources.exists() {
        return Err(reported(
            stderr,
            DxError::usage(format!("sources missing: {}", sources.display())),
        ));
    }
    let mut deps = match crate::consistency::load_manifest(eco, manifest) {
        Ok(deps) => deps,
        Err(err) => {
            return Err(reported(stderr, err.into()));
        }
    };
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let dep_names: Vec<String> = deps.keys().cloned().collect();
    let usages = match find_usages(eco, sources, &dep_names) {
        Ok(usages) => usages,
        Err(err) => {
            return Err(reported(stderr, err.into()));
        }
    };
    let mut failures = Vec::new();
    for (name, info) in &deps {
        let usage = usages.get(name).copied().unwrap_or_default();
        let used_any = usage.src || usage.test || usage.build;
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
        return Err(DxError::failure(format!(
            "{} usage failures ({})",
            failures.len(),
            eco.name()
        )));
    }
    let _ = writeln!(
        stdout,
        "depcheck: OK: usage: {} declarations used in owning scope ({})",
        deps.len(),
        eco.name()
    );
    Ok(())
}
