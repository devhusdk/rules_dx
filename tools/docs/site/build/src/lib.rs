//! Assembles one mdBook source tree and runs the pinned mdBook build.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One source page of the book and the route it renders under.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    pub route: String,
    pub path: String,
}

/// The inputs of one site build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub mdbook: String,
    pub book: String,
    pub summary: String,
    pub manifest: String,
    pub out: String,
    pub api_tree: String,
    pub api_summary: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> String {
    let mut line = String::from("usage: site_build --mdbook FILE --book FILE --summary FILE");
    line.push_str(" --manifest FILE --out DIR [--api-tree DIR --api-summary FILE]");
    line
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].clone();
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        match flag.as_str() {
            "--mdbook" => options.mdbook = value.clone(),
            "--book" => options.book = value.clone(),
            "--summary" => options.summary = value.clone(),
            "--manifest" => options.manifest = value.clone(),
            "--out" => options.out = value.clone(),
            "--api-tree" => options.api_tree = value.clone(),
            "--api-summary" => options.api_summary = value.clone(),
            _ => return Err(format!("unknown argument '{flag}'")),
        }
        index += 2;
    }
    for (flag, value) in [
        ("--mdbook", &options.mdbook),
        ("--book", &options.book),
        ("--summary", &options.summary),
        ("--manifest", &options.manifest),
        ("--out", &options.out),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    if options.api_tree.is_empty() != options.api_summary.is_empty() {
        return Err("--api-tree and --api-summary go together".to_string());
    }
    Ok(options)
}

/// Validates one source route, returning "" when mdBook can render it.
pub fn route_error(route: &str) -> String {
    if route.is_empty() {
        return "docs_site: a source route is empty".to_string();
    }
    if !route.ends_with(".md") {
        return format!("docs_site: route '{route}' is not a Markdown page");
    }
    if route.starts_with('/') || route.contains('\\') || route.contains(char::is_whitespace) {
        return format!("docs_site: route '{route}' is not a relative clean path");
    }
    for part in route.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return format!("docs_site: route '{route}' has an empty or relative segment");
        }
    }
    String::new()
}

/// Validates one page list, returning "" when mdBook can render every route once.
pub fn pages_error(pages: &[Page]) -> String {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for page in pages {
        let error = route_error(&page.route);
        if !error.is_empty() {
            return error;
        }
        if page.path.is_empty() {
            return format!("docs_site: route '{}' names no source page", page.route);
        }
        if !seen.insert(page.route.as_str()) {
            return format!("docs_site: route '{}' is declared twice", page.route);
        }
    }
    String::new()
}

/// Returns the alias mdBook's own link rewriting needs for one route.
///
/// mdBook renders `dir/README.md` as `dir/index.html` but rewrites a link to
/// `dir/README.md` as `dir/README.html`, so the rendered index is copied.
pub fn alias_route(route: &str) -> Option<String> {
    let name = route.strip_suffix(".md")?;
    if name != "README" && !name.ends_with("/README") {
        return None;
    }
    Some(format!("{name}.html"))
}

/// Parses one page manifest, returning "" when the text holds no page line.
pub fn manifest_error(text: &str) -> String {
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((route, path)) = line.split_once('\t') else {
            return format!("docs_site: page line needs a route and a path: {line}");
        };
        if path.is_empty() {
            return format!("docs_site: page line for route '{route}' names no path");
        }
    }
    String::new()
}

/// Returns the pages one manifest text declares, in file order.
pub fn manifest_pages(text: &str) -> Vec<Page> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            line.split_once('\t').map(|(route, path)| Page {
                route: route.to_string(),
                path: path.to_string(),
            })
        })
        .collect()
}

/// Returns the first mdBook warning or error line, or "" when the run was clean.
///
/// mdBook exits 0 after logging a warning, so its log decides whether the
/// render is trustworthy.
pub fn log_error(log: &str) -> String {
    for line in log.lines() {
        for marker in ["[WARN]", "[ERROR]"] {
            if let Some(at) = line.find(marker) {
                let text = line[at..].trim();
                return format!("mdbook reported {text}");
            }
        }
    }
    String::new()
}

/// Assembles the book and runs mdBook, returning "" on success.
pub fn run(options: &Options) -> String {
    let manifest = read(&options.manifest);
    if manifest.is_empty() {
        return format!(
            "docs_site: page manifest '{}' is unreadable",
            options.manifest
        );
    }
    let error = manifest_error(&manifest);
    if !error.is_empty() {
        return error;
    }
    let mut pages = manifest_pages(&manifest);
    if !options.api_tree.is_empty() {
        match api_pages(Path::new(&options.api_tree)) {
            Ok(api) => pages.extend(api),
            Err(error) => return error,
        }
    }
    let error = pages_error(&pages);
    if !error.is_empty() {
        return error;
    }
    let scratch = scratch_dir();
    let book = scratch.join("book");
    if !scratch_is_usable(&scratch) {
        return format!(
            "docs_site: cannot stage the book under '{}'",
            scratch.display()
        );
    }
    let staging = stage(&book, options, &pages);
    if let Err(error) = staging {
        let _ = fs::remove_dir_all(&scratch);
        return error;
    }
    let out = absolute(&options.out);
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let build = Command::new(&options.mdbook)
        .arg("build")
        .arg("--dest-dir")
        .arg(&out)
        .arg(&book)
        .output();
    let _ = fs::remove_dir_all(&scratch);
    let build = match build {
        Ok(build) => build,
        Err(error) => return format!("cannot run mdbook '{}': {error}", options.mdbook),
    };
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let reported = log_error(&log);
    if !reported.is_empty() {
        return reported;
    }
    if !build.status.success() {
        return format!("mdbook {} exited with {}", options.mdbook, build.status);
    }
    alias_errors(&pages, &out)
}

fn stage(book: &Path, options: &Options, pages: &[Page]) -> Result<(), String> {
    let src = book.join("src");
    fs::create_dir_all(&src)
        .map_err(|error| format!("docs_site: cannot create '{}': {error}", src.display()))?;
    copy(Path::new(&options.book), &book.join("book.toml"))?;
    let summary = src.join("SUMMARY.md");
    let mut text = fs::read_to_string(&options.summary).map_err(|error| {
        format!(
            "docs_site: cannot stage '{}' as '{}': {error}",
            options.summary,
            summary.display()
        )
    })?;
    if !options.api_summary.is_empty() {
        let fragment = fs::read_to_string(&options.api_summary).map_err(|error| {
            format!(
                "docs_site: cannot read API summary '{}': {error}",
                options.api_summary
            )
        })?;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&fragment);
    }
    fs::write(&summary, text)
        .map_err(|error| format!("docs_site: cannot write '{}': {error}", summary.display()))?;
    for page in pages {
        let to = src.join(&page.route);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("docs_site: cannot create '{}': {error}", parent.display())
            })?;
        }
        copy(Path::new(&page.path), &to)?;
    }
    Ok(())
}

/// Returns one page per Markdown file of an API tree, routed under `api/`.
pub fn api_pages(tree: &Path) -> Result<Vec<Page>, String> {
    let mut pages = Vec::new();
    let mut pending = vec![tree.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| {
            format!(
                "docs_site: cannot read API tree '{}': {error}",
                dir.display()
            )
        })?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("docs_site: cannot read API tree: {error}"))?
                .path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(tree)
                .map_err(|_| format!("docs_site: '{}' is outside the API tree", path.display()))?;
            pages.push(Page {
                route: format!("api/{}", rel.to_string_lossy().replace('\\', "/")),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    pages.sort();
    if pages.is_empty() {
        return Err(format!(
            "docs_site: API tree '{}' holds no page",
            tree.display()
        ));
    }
    Ok(pages)
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to).map_err(|error| {
        format!(
            "docs_site: cannot stage '{}' as '{}': {error}",
            from.display(),
            to.display()
        )
    })?;
    Ok(())
}

fn alias_errors(pages: &[Page], out: &Path) -> String {
    for page in pages {
        let Some(alias) = alias_route(&page.route) else {
            continue;
        };
        let target = out.join(&alias);
        let index = target.with_file_name("index.html");
        if !index.is_file() {
            return format!(
                "docs_site: mdbook rendered no '{}' for route '{}'",
                index.display(),
                page.route
            );
        }
        if let Err(error) = fs::copy(&index, &target) {
            return format!("docs_site: cannot alias '{}': {error}", target.display());
        }
    }
    String::new()
}

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    std::env::temp_dir().join(format!("dx-site-build-{}-{nanos}", std::process::id()))
}

fn scratch_is_usable(scratch: &Path) -> bool {
    if scratch.exists() {
        let _ = fs::remove_dir_all(scratch);
    }
    fs::create_dir_all(scratch).is_ok()
}

fn absolute(path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return candidate;
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(candidate),
        Err(_) => candidate,
    }
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
