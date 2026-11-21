//! Assembles one mdBook source tree and runs the pinned mdBook build.

#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf, StripPrefixError};
use std::process::{Command, ExitStatus};

use dx_atomic_fs::path_io::{self, PathIoError};

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

/// Runs `site_build` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return 0;
    }
    let options = match parse(args) {
        Ok(options) => options,
        Err(detail) => {
            eprintln!("site_build: {detail}\n{}", usage());
            return 2;
        }
    };
    match run(&options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("site_build: {error}");
            1
        }
    }
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> String {
    let mut line = String::from("usage: site_build --mdbook FILE --book FILE --summary FILE");
    line.push_str(" --manifest FILE --out DIR [--api-tree DIR --api-summary FILE]");
    line
}

/// A rejected command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgsError {
    MissingValue { flag: String },
    Unknown { flag: String },
    Required { flag: String },
    ApiPair,
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { flag } => write!(f, "{flag} needs a value"),
            Self::Unknown { flag } => write!(f, "unknown argument '{flag}'"),
            Self::Required { flag } => write!(f, "{flag} is required"),
            Self::ApiPair => f.write_str("--api-tree and --api-summary go together"),
        }
    }
}

impl std::error::Error for ArgsError {}

/// Why the book cannot be assembled or rendered.
#[derive(Debug)]
pub enum BuildError {
    EmptyRoute,
    NotMarkdown {
        route: String,
    },
    NotClean {
        route: String,
    },
    BadSegment {
        route: String,
    },
    NoSource {
        route: String,
    },
    DuplicateRoute {
        route: String,
    },
    ManifestLine {
        line: String,
    },
    ManifestPath {
        route: String,
    },
    EmptyManifest {
        path: String,
    },
    Io(PathIoError),
    OutsideApiTree {
        path: PathBuf,
        source: StripPrefixError,
    },
    EmptyApiTree {
        tree: PathBuf,
    },
    Staging(io::Error),
    Stage {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
    RunMdbook {
        mdbook: String,
        source: io::Error,
    },
    MdbookReported {
        line: String,
    },
    MdbookExit {
        mdbook: String,
        status: ExitStatus,
    },
    NoRenderedIndex {
        index: PathBuf,
        route: String,
    },
    Alias {
        target: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoute => f.write_str("docs_site: a source route is empty"),
            Self::NotMarkdown { route } => {
                write!(f, "docs_site: route '{route}' is not a Markdown page")
            }
            Self::NotClean { route } => {
                write!(f, "docs_site: route '{route}' is not a relative clean path")
            }
            Self::BadSegment { route } => write!(
                f,
                "docs_site: route '{route}' has an empty or relative segment"
            ),
            Self::NoSource { route } => {
                write!(f, "docs_site: route '{route}' names no source page")
            }
            Self::DuplicateRoute { route } => {
                write!(f, "docs_site: route '{route}' is declared twice")
            }
            Self::ManifestLine { line } => {
                write!(f, "docs_site: page line needs a route and a path: {line}")
            }
            Self::ManifestPath { route } => {
                write!(f, "docs_site: page line for route '{route}' names no path")
            }
            Self::EmptyManifest { path } => {
                write!(f, "docs_site: page manifest '{path}' holds no page")
            }
            Self::Io(error) => write!(f, "docs_site: {error}"),
            Self::OutsideApiTree { path, source } => write!(
                f,
                "docs_site: '{}' is outside the API tree: {source}",
                path.display()
            ),
            Self::EmptyApiTree { tree } => {
                write!(f, "docs_site: API tree '{}' holds no page", tree.display())
            }
            Self::Staging(error) => {
                write!(f, "docs_site: cannot create a staging directory: {error}")
            }
            Self::Stage { from, to, source } => write!(
                f,
                "docs_site: cannot stage '{}' as '{}': {source}",
                from.display(),
                to.display()
            ),
            Self::RunMdbook { mdbook, source } => {
                write!(f, "cannot run mdbook '{mdbook}': {source}")
            }
            Self::MdbookReported { line } => write!(f, "mdbook reported {line}"),
            Self::MdbookExit { mdbook, status } => {
                write!(f, "mdbook {mdbook} exited with {status}")
            }
            Self::NoRenderedIndex { index, route } => write!(
                f,
                "docs_site: mdbook rendered no '{}' for route '{route}'",
                index.display()
            ),
            Self::Alias { target, source } => {
                write!(
                    f,
                    "docs_site: cannot alias '{}': {source}",
                    target.display()
                )
            }
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::OutsideApiTree { source, .. } => Some(source),
            Self::Staging(source)
            | Self::Stage { source, .. }
            | Self::RunMdbook { source, .. }
            | Self::Alias { source, .. } => Some(source),
            Self::EmptyRoute
            | Self::NotMarkdown { .. }
            | Self::NotClean { .. }
            | Self::BadSegment { .. }
            | Self::NoSource { .. }
            | Self::DuplicateRoute { .. }
            | Self::ManifestLine { .. }
            | Self::ManifestPath { .. }
            | Self::EmptyManifest { .. }
            | Self::EmptyApiTree { .. }
            | Self::MdbookReported { .. }
            | Self::MdbookExit { .. }
            | Self::NoRenderedIndex { .. } => None,
        }
    }
}

impl From<PathIoError> for BuildError {
    fn from(error: PathIoError) -> Self {
        Self::Io(error)
    }
}

/// Parses argv into inputs.
///
/// # Errors
///
/// Returns an error for an unknown flag, a flag without a value, a missing
/// required flag or an API tree without its summary.
pub fn parse(args: &[String]) -> Result<Options, ArgsError> {
    let mut options = Options::default();
    let mut index = 0;
    while let Some(flag) = args.get(index).cloned() {
        let Some(value) = args.get(index + 1) else {
            return Err(ArgsError::MissingValue { flag });
        };
        match flag.as_str() {
            "--mdbook" => options.mdbook.clone_from(value),
            "--book" => options.book.clone_from(value),
            "--summary" => options.summary.clone_from(value),
            "--manifest" => options.manifest.clone_from(value),
            "--out" => options.out.clone_from(value),
            "--api-tree" => options.api_tree.clone_from(value),
            "--api-summary" => options.api_summary.clone_from(value),
            _ => return Err(ArgsError::Unknown { flag }),
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
            return Err(ArgsError::Required {
                flag: flag.to_owned(),
            });
        }
    }
    if options.api_tree.is_empty() != options.api_summary.is_empty() {
        return Err(ArgsError::ApiPair);
    }
    Ok(options)
}

/// Validates one source route.
///
/// # Errors
///
/// Returns an error when mdBook cannot render the route.
pub fn check_route(route: &str) -> Result<(), BuildError> {
    if route.is_empty() {
        return Err(BuildError::EmptyRoute);
    }
    if !Path::new(route)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        return Err(BuildError::NotMarkdown {
            route: route.to_owned(),
        });
    }
    if route.starts_with('/') || route.contains('\\') || route.contains(char::is_whitespace) {
        return Err(BuildError::NotClean {
            route: route.to_owned(),
        });
    }
    for part in route.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(BuildError::BadSegment {
                route: route.to_owned(),
            });
        }
    }
    Ok(())
}

/// Validates one page list.
///
/// # Errors
///
/// Returns an error unless mdBook can render every route once.
pub fn check_pages(pages: &[Page]) -> Result<(), BuildError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for page in pages {
        check_route(&page.route)?;
        if page.path.is_empty() {
            return Err(BuildError::NoSource {
                route: page.route.clone(),
            });
        }
        if !seen.insert(page.route.as_str()) {
            return Err(BuildError::DuplicateRoute {
                route: page.route.clone(),
            });
        }
    }
    Ok(())
}

/// Returns the alias mdBook's own link rewriting needs for one route.
///
/// mdBook renders `dir/README.md` as `dir/index.html` but rewrites a link to
/// `dir/README.md` as `dir/README.html`, so the rendered index is copied.
#[must_use]
pub fn alias_route(route: &str) -> Option<String> {
    let name = route.strip_suffix(".md")?;
    if name != "README" && !name.ends_with("/README") {
        return None;
    }
    Some(format!("{name}.html"))
}

/// Validates the page lines of one manifest.
///
/// # Errors
///
/// Returns an error for a line without a route and a path.
pub fn check_manifest(text: &str) -> Result<(), BuildError> {
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((route, path)) = line.split_once('\t') else {
            return Err(BuildError::ManifestLine {
                line: line.to_owned(),
            });
        };
        if path.is_empty() {
            return Err(BuildError::ManifestPath {
                route: route.to_owned(),
            });
        }
    }
    Ok(())
}

/// Returns the pages one manifest text declares, in file order.
#[must_use]
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

/// Checks an mdBook log for a warning or error line.
///
/// mdBook exits 0 after logging a warning, so its log decides whether the
/// render is trustworthy.
///
/// # Errors
///
/// Returns the first warning or error line mdBook logged.
pub fn check_log(log: &str) -> Result<(), BuildError> {
    for line in log.lines() {
        for marker in ["[WARN]", "[ERROR]"] {
            if let Some((_, after)) = line.split_once(marker) {
                let text = format!("{marker}{after}");
                return Err(BuildError::MdbookReported {
                    line: text.trim().to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// Assembles the book and runs mdBook.
///
/// # Errors
///
/// Returns an error when an input is unreadable or invalid, staging fails,
/// mdBook fails or logs a warning, or a README alias cannot be written.
pub fn run(options: &Options) -> Result<(), BuildError> {
    let manifest = path_io::read_to_string(Path::new(&options.manifest))?;
    if manifest.is_empty() {
        return Err(BuildError::EmptyManifest {
            path: options.manifest.clone(),
        });
    }
    check_manifest(&manifest)?;
    let mut pages = manifest_pages(&manifest);
    if !options.api_tree.is_empty() {
        pages.extend(api_pages(Path::new(&options.api_tree))?);
    }
    check_pages(&pages)?;
    let scratch = tempfile::Builder::new()
        .prefix("dx-site-build-")
        .tempdir()
        .map_err(BuildError::Staging)?;
    let book = scratch.path().join("book");
    stage(&book, options, &pages)?;
    let out = absolute(&options.out);
    if let Some(parent) = out.parent() {
        path_io::create_dir_all(parent)?;
    }
    let build = Command::new(&options.mdbook)
        .arg("build")
        .arg("--dest-dir")
        .arg(&out)
        .arg(&book)
        .output();
    drop(scratch);
    let build = build.map_err(|source| BuildError::RunMdbook {
        mdbook: options.mdbook.clone(),
        source,
    })?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    check_log(&log)?;
    if !build.status.success() {
        return Err(BuildError::MdbookExit {
            mdbook: options.mdbook.clone(),
            status: build.status,
        });
    }
    write_aliases(&pages, &out)
}

fn stage(book: &Path, options: &Options, pages: &[Page]) -> Result<(), BuildError> {
    let src = book.join("src");
    path_io::create_dir_all(&src)?;
    copy(Path::new(&options.book), &book.join("book.toml"))?;
    let summary = src.join("SUMMARY.md");
    let mut text = path_io::read_to_string(Path::new(&options.summary))?;
    if !options.api_summary.is_empty() {
        let fragment = path_io::read_to_string(Path::new(&options.api_summary))?;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&fragment);
    }
    path_io::write_scratch(&summary, text.as_bytes())?;
    for page in pages {
        let to = src.join(&page.route);
        if let Some(parent) = to.parent() {
            path_io::create_dir_all(parent)?;
        }
        copy(Path::new(&page.path), &to)?;
    }
    Ok(())
}

/// Returns one page per Markdown file of an API tree, routed under `api/`.
///
/// # Errors
///
/// Returns an error when the tree cannot be listed or holds no page.
pub fn api_pages(tree: &Path) -> Result<Vec<Page>, BuildError> {
    let mut pages = Vec::new();
    let mut pending = vec![tree.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in path_io::read_dir(&dir)? {
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(tree)
                .map_err(|source| BuildError::OutsideApiTree {
                    path: path.clone(),
                    source,
                })?;
            pages.push(Page {
                route: format!("api/{}", rel.to_string_lossy().replace('\\', "/")),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    pages.sort();
    if pages.is_empty() {
        return Err(BuildError::EmptyApiTree {
            tree: tree.to_path_buf(),
        });
    }
    Ok(pages)
}

fn copy(from: &Path, to: &Path) -> Result<(), BuildError> {
    fs::copy(from, to).map_err(|source| BuildError::Stage {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn write_aliases(pages: &[Page], out: &Path) -> Result<(), BuildError> {
    for page in pages {
        let Some(alias) = alias_route(&page.route) else {
            continue;
        };
        let target = out.join(&alias);
        let index = target.with_file_name("index.html");
        if !index.is_file() {
            return Err(BuildError::NoRenderedIndex {
                index,
                route: page.route.clone(),
            });
        }
        fs::copy(&index, &target).map_err(|source| BuildError::Alias { target, source })?;
    }
    Ok(())
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

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
