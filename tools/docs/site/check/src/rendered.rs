//! Checks that one rendered mdBook site holds together as a browsable website.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

use documentation_ir::tool::{PathIoError, path_io};

/// One rendered site tree to inspect.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    pub root: PathBuf,
    pub landing: bool,
}

/// One page of a rendered site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub route: String,
    pub title: String,
    pub ids: Vec<String>,
    pub links: Vec<String>,
    pub assets: Vec<String>,
    pub has_viewport: bool,
    pub has_nav: bool,
    pub headings: Vec<String>,
    pub list_items: usize,
    pub tables: usize,
    pub code_blocks: usize,
    pub highlighted: usize,
}

/// The usage line for one rendered-site inspection.
#[must_use]
pub fn usage() -> String {
    let mut line = String::from("usage: site_check --rendered DIR [--landing] [--forbid TEXT]");
    line.push_str("...");
    line
}

/// Every `.html` page of one rendered site, sorted by route.
///
/// # Errors
///
/// Returns the first directory that cannot be listed or page that cannot be read.
pub fn pages(root: &Path) -> Result<Vec<Page>, PathIoError> {
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Page>) -> Result<(), PathIoError> {
    for path in path_io::read_dir(dir)? {
        if path.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "html") {
            out.push(page(root, &path)?);
        }
    }
    Ok(())
}

fn page(root: &Path, path: &Path) -> Result<Page, PathIoError> {
    let route = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let html = path_io::read_to_string(path)?;
    Ok(Page {
        route,
        title: first_between(&html, "<title>", "</title>"),
        ids: attributes(&html, "id"),
        links: hrefs(&html),
        assets: srcs(&html).into_iter().chain(hrefs(&html)).collect(),
        has_viewport: html.contains("name=\"viewport\""),
        has_nav: html.contains("<nav"),
        headings: headings(&html),
        list_items: html.matches("<li>").count(),
        tables: html.matches("<table").count(),
        code_blocks: html.matches("<pre><code").count(),
        highlighted: html.matches("class=\"language-").count(),
    })
}

fn first_between(html: &str, open: &str, close: &str) -> String {
    let Some(rest) = html.split_once(open).map(|(_, tail)| tail) else {
        return String::new();
    };
    rest.split_once(close)
        .map(|(head, _)| head.trim().to_string())
        .unwrap_or_default()
}

fn headings(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some((_, tail)) = rest.split_once("<h1") {
        out.push(heading_text(tail));
        rest = tail;
    }
    rest = html;
    while let Some((_, tail)) = rest.split_once("<h2") {
        out.push(heading_text(tail));
        rest = tail;
    }
    out.retain(|text| !text.is_empty());
    out
}

fn heading_text(tail: &str) -> String {
    let Some((_, rest)) = tail.split_once('>') else {
        return String::new();
    };
    let Some((inner, _)) = rest.split_once("</h") else {
        return String::new();
    };
    let mut text = String::new();
    let mut open = false;
    for ch in inner.chars() {
        match ch {
            '<' => open = true,
            '>' => open = false,
            _ if !open => text.push(ch),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hrefs(html: &str) -> Vec<String> {
    attribute_values(html, "href")
}

fn srcs(html: &str) -> Vec<String> {
    attribute_values(html, "src")
}

fn attributes(html: &str, name: &str) -> Vec<String> {
    let needle = format!("{name}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some((_, tail)) = rest.split_once(needle.as_str()) {
        let Some((value, remainder)) = tail.split_once('"') else {
            break;
        };
        out.push(value.to_string());
        rest = remainder;
    }
    out
}

fn attribute_values(html: &str, name: &str) -> Vec<String> {
    let needle = format!("{name}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some((head, tail)) = rest.split_once(needle.as_str()) {
        let before = head.rsplit('<').next().unwrap_or("");
        let Some((value, remainder)) = tail.split_once('"') else {
            break;
        };
        if !before.contains("data-") {
            out.push(value.to_string());
        }
        rest = remainder;
    }
    out
}

/// Splits one rendered target into its route and its anchor.
#[must_use]
pub fn split_target(target: &str) -> (String, String) {
    match target.split_once('#') {
        Some((route, anchor)) => (route.to_string(), anchor.to_string()),
        None => (target.to_string(), String::new()),
    }
}

/// Returns True when one link target never leaves the site.
#[must_use]
pub fn is_internal(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('#')
        && !target.starts_with("mailto:")
        && !target.contains("://")
        && !target.starts_with("//")
}

/// Percent-decodes one URL fragment, or None when the encoding is malformed.
#[must_use]
pub fn decode_fragment(fragment: &str) -> Option<String> {
    let bytes = fragment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let pair = fragment.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(pair, 16).ok()?);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Resolves one relative link to the route it addresses inside the site.
#[must_use]
pub fn resolve(from: &str, target: &str) -> String {
    let base = from.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut segments: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    let joined = segments.join("/");
    if target.ends_with('/') {
        format!("{joined}/")
    } else {
        joined
    }
}

/// Returns the file one resolved link addresses inside the site.
#[must_use]
pub fn page_name(root: &Path, target: &str) -> String {
    if target.is_empty() || target == "./" || target == "." {
        return "index.html".to_string();
    }
    let trimmed = target.trim_end_matches('/');
    if trimmed.is_empty() {
        return "index.html".to_string();
    }
    if target.ends_with('/') || root.join(trimmed).is_dir() {
        return format!("{trimmed}/index.html");
    }
    trimmed.to_string()
}

/// Returns True when one page is a fragment the site serves beside its chapters.
#[must_use]
pub fn is_fragment(route: &str) -> bool {
    matches!(route, "toc.html" | "print.html" | "404.html")
}

/// Why one logical asset does not resolve to a single file.
#[derive(Debug)]
pub enum AssetError {
    List {
        logical: String,
        source: PathIoError,
    },
    Missing {
        logical: String,
    },
    Several {
        logical: String,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::List { logical, source } => {
                write!(f, "rendered site has no {logical}: {source}")
            }
            Self::Missing { logical } => write!(f, "rendered site has no {logical}"),
            Self::Several { logical } => write!(f, "rendered site has several {logical} files"),
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::List { source, .. } => Some(source),
            Self::Missing { .. } | Self::Several { .. } => None,
        }
    }
}

/// Resolves one logical asset path such as `css/general.css` to the single exact or
/// `<stem>-<hex>.<ext>` file under `root`.
///
/// # Errors
///
/// Returns an error when the directory cannot be listed or holds no match or several.
pub fn asset(root: &Path, logical: &str) -> Result<PathBuf, AssetError> {
    let (dir, name) = logical.rsplit_once('/').unwrap_or(("", logical));
    let (stem, ext) = name.split_once('.').unwrap_or((name, ""));
    let dir = root.join(dir);
    let mut found: Vec<PathBuf> = Vec::new();
    let entries = path_io::read_dir(&dir).map_err(|source| AssetError::List {
        logical: logical.to_owned(),
        source,
    })?;
    for path in entries {
        let file = path
            .file_name()
            .map(|file| file.to_string_lossy().into_owned())
            .unwrap_or_default();
        let hashed = file
            .strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|rest| rest.strip_suffix(ext))
            .and_then(|rest| rest.strip_suffix('.'))
            .is_some_and(|hash| !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit()));
        if (file == name || hashed) && path.is_file() {
            found.push(path);
        }
    }
    match found.len() {
        0 => Err(AssetError::Missing {
            logical: logical.to_owned(),
        }),
        1 => Ok(found.remove(0)),
        _ => Err(AssetError::Several {
            logical: logical.to_owned(),
        }),
    }
}

/// One reason a rendered site does not hold together.
#[derive(Debug)]
pub enum RenderedFinding {
    Read(PathIoError),
    Asset(AssetError),
    NoPages,
    NoIndex,
    NoPrint,
    NoSearchObject,
    NoTitle { route: String },
    NoHeading { route: String },
    NoViewport { route: String },
    NoNavigation { route: String },
    EmptyLink { route: String },
    MissingLink { route: String, link: String },
    MalformedAnchor { route: String, link: String },
    MissingAnchor { route: String, link: String },
    MissingAsset { route: String, asset: String },
    NoSearchToggle,
    NoSidebarToggle,
    NoCodeBlock,
    NoHighlightedCode,
    NoList,
    RepeatedRoute { route: String },
    Publishes { route: String, needle: String },
}

impl fmt::Display for RenderedFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::Asset(error) => error.fmt(f),
            Self::NoPages => f.write_str("rendered site has no HTML page"),
            Self::NoIndex => f.write_str("rendered site has no index.html at its root"),
            Self::NoPrint => f.write_str("rendered site has no print.html"),
            Self::NoSearchObject => f.write_str("the search index holds no search object"),
            Self::NoTitle { route } => write!(f, "{route} has no <title>"),
            Self::NoHeading { route } => write!(f, "{route} has no heading"),
            Self::NoViewport { route } => write!(f, "{route} has no viewport meta tag"),
            Self::NoNavigation { route } => write!(f, "{route} has no navigation element"),
            Self::EmptyLink { route } => write!(f, "{route} links to an empty target"),
            Self::MissingLink { route, link } => write!(f, "{route} links to missing '{link}'"),
            Self::MalformedAnchor { route, link } => {
                write!(f, "{route} links to malformed anchor encoding '{link}'")
            }
            Self::MissingAnchor { route, link } => {
                write!(f, "{route} links to missing anchor '{link}'")
            }
            Self::MissingAsset { route, asset } => {
                write!(f, "{route} references missing asset '{asset}'")
            }
            Self::NoSearchToggle => f.write_str("index.html has no search toggle"),
            Self::NoSidebarToggle => f.write_str("index.html has no sidebar toggle"),
            Self::NoCodeBlock => f.write_str("index.html renders no code block"),
            Self::NoHighlightedCode => f.write_str("index.html renders no highlighted code block"),
            Self::NoList => f.write_str("index.html renders no list"),
            Self::RepeatedRoute { route } => write!(f, "{route} repeats a route"),
            Self::Publishes { route, needle } => write!(f, "{route} publishes {needle:?}"),
        }
    }
}

fn check_page(
    root: &Path,
    page: &Page,
    ids: &BTreeMap<String, Vec<String>>,
    errors: &mut Vec<RenderedFinding>,
) {
    let route = || page.route.clone();
    if !is_fragment(&page.route) {
        if page.title.is_empty() {
            errors.push(RenderedFinding::NoTitle { route: route() });
        }
        if page.headings.is_empty() {
            errors.push(RenderedFinding::NoHeading { route: route() });
        }
        if !page.has_viewport {
            errors.push(RenderedFinding::NoViewport { route: route() });
        }
        if !page.has_nav {
            errors.push(RenderedFinding::NoNavigation { route: route() });
        }
    }
    for link in &page.links {
        if !is_internal(link) && !link.starts_with('#') && !link.starts_with('?') {
            continue;
        }
        let (target, anchor) = split_target(link);
        let target = target.split('?').next().unwrap_or("").to_string();
        if target.is_empty() && !link.contains('#') && !link.starts_with('?') {
            errors.push(RenderedFinding::EmptyLink { route: route() });
            continue;
        }
        let name = if target.is_empty() {
            page.route.clone()
        } else {
            let target = resolve(&page.route, &target);
            let name = page_name(root, &target);
            if !root.join(&name).is_file() {
                errors.push(RenderedFinding::MissingLink {
                    route: route(),
                    link: link.clone(),
                });
                continue;
            }
            name
        };
        if anchor.is_empty() || anchor == "top" || anchor == "mdbook-searchbar" {
            continue;
        }
        let found = ids.get(&name);
        let has = |id: &str| found.is_some_and(|found| found.iter().any(|f| f == id));
        let decoded = decode_fragment(&anchor);
        if has(&anchor) || decoded.as_deref().is_some_and(has) {
            continue;
        }
        if decoded.is_none() {
            errors.push(RenderedFinding::MalformedAnchor {
                route: route(),
                link: link.clone(),
            });
        } else {
            errors.push(RenderedFinding::MissingAnchor {
                route: route(),
                link: link.clone(),
            });
        }
    }
    for asset in &page.assets {
        if !is_internal(asset) {
            continue;
        }
        let (target, _) = split_target(asset);
        let target = target.split('?').next().unwrap_or("");
        if target.is_empty() {
            continue;
        }
        let target = resolve(&page.route, target);
        let name = page_name(root, &target);
        if !root.join(&name).exists() {
            errors.push(RenderedFinding::MissingAsset {
                route: route(),
                asset: asset.clone(),
            });
        }
    }
}

/// Returns the findings for one rendered site, empty when it holds.
#[must_use]
pub fn check_rendered(rendered: &Rendered, forbid: &[String]) -> Vec<RenderedFinding> {
    let root = rendered.root.as_path();
    let pages = match pages(root) {
        Ok(pages) => pages,
        Err(error) => return vec![RenderedFinding::Read(error)],
    };
    if pages.is_empty() {
        return vec![RenderedFinding::NoPages];
    }
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for page in &pages {
        ids.insert(page.route.clone(), page.ids.clone());
    }

    if !root.join("index.html").is_file() {
        return vec![RenderedFinding::NoIndex];
    }
    let search_index = match asset(root, "searchindex.js") {
        Ok(path) => path,
        Err(error) => return vec![RenderedFinding::Asset(error)],
    };
    for logical in ["css/general.css", "book.js"] {
        if let Err(error) = asset(root, logical) {
            return vec![RenderedFinding::Asset(error)];
        }
    }
    if !root.join("print.html").is_file() {
        return vec![RenderedFinding::NoPrint];
    }

    let search = match path_io::read_to_string(&search_index) {
        Ok(search) => search,
        Err(error) => return vec![RenderedFinding::Read(error)],
    };
    if !search.contains("window.search") {
        return vec![RenderedFinding::NoSearchObject];
    }

    let mut errors: Vec<RenderedFinding> = Vec::new();
    for page in &pages {
        check_page(root, page, &ids, &mut errors);
    }

    let index = pages
        .iter()
        .find(|page| page.route == "index.html")
        .cloned()
        .unwrap_or_default();
    if !index.ids.iter().any(|id| id == "mdbook-search-toggle") {
        errors.push(RenderedFinding::NoSearchToggle);
    }
    if !index.ids.iter().any(|id| id == "mdbook-sidebar-toggle") {
        errors.push(RenderedFinding::NoSidebarToggle);
    }
    if rendered.landing {
        if index.code_blocks == 0 {
            errors.push(RenderedFinding::NoCodeBlock);
        }
        if index.highlighted == 0 {
            errors.push(RenderedFinding::NoHighlightedCode);
        }
        if index.list_items == 0 {
            errors.push(RenderedFinding::NoList);
        }
    }

    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for page in &pages {
        if seen
            .insert(page.route.as_str(), page.title.as_str())
            .is_some()
        {
            errors.push(RenderedFinding::RepeatedRoute {
                route: page.route.clone(),
            });
        }
    }

    for needle in forbid {
        if needle.is_empty() {
            continue;
        }
        match first_publisher(root, &pages, needle) {
            Ok(Some(page)) => errors.push(RenderedFinding::Publishes {
                route: page.route.clone(),
                needle: needle.clone(),
            }),
            Ok(None) => {}
            Err(error) => errors.push(RenderedFinding::Read(error)),
        }
    }

    errors
}

/// The first page whose HTML contains `needle`.
fn first_publisher<'a>(
    root: &Path,
    pages: &'a [Page],
    needle: &str,
) -> Result<Option<&'a Page>, PathIoError> {
    for page in pages {
        if path_io::read_to_string(&root.join(&page.route))?.contains(needle) {
            return Ok(Some(page));
        }
    }
    Ok(None)
}

/// Returns a report naming one route per rendered page, for negative-case fixtures.
#[must_use]
pub fn route_report(rendered: &Rendered) -> String {
    let pages = match pages(rendered.root.as_path()) {
        Ok(pages) => pages,
        Err(error) => return error.to_string(),
    };
    let mut text = String::new();
    for page in pages {
        writeln!(
            text,
            "{}: title={} headings={} code={} highlighted={} lists={} tables={}",
            page.route,
            page.title,
            page.headings.len(),
            page.code_blocks,
            page.highlighted,
            page.list_items,
            page.tables
        )
        .unwrap_or_default();
    }
    text
}

#[cfg(test)]
#[path = "rendered_tests.rs"]
mod rendered_tests;
