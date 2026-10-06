//! Checks that one rendered mdBook site holds together as a browsable website.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

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
pub fn usage() -> String {
    let mut line = String::from("usage: site_check --rendered DIR [--landing] [--forbid TEXT]");
    line.push_str("...");
    line
}

/// Every `.html` page of one rendered site, sorted by route.
pub fn pages(root: &Path) -> Vec<Page> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.route.cmp(&b.route));
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Page>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "html") {
            if let Some(page) = page(root, &path) {
                out.push(page);
            }
        }
    }
}

fn page(root: &Path, path: &Path) -> Option<Page> {
    let route = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let html = fs::read_to_string(path).ok()?;
    Some(Page {
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
    while let Some(at) = rest.find(needle.as_str()) {
        let before = rest[..at].rsplit('<').next().unwrap_or("");
        let tail = &rest[at + needle.len()..];
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
pub fn split_target(target: &str) -> (String, String) {
    match target.split_once('#') {
        Some((route, anchor)) => (route.to_string(), anchor.to_string()),
        None => (target.to_string(), String::new()),
    }
}

/// Returns True when one link target never leaves the site.
pub fn is_internal(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('#')
        && !target.starts_with("mailto:")
        && !target.contains("://")
        && !target.starts_with("//")
}

/// Resolves one relative link to the route it addresses inside the site.
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
pub fn is_fragment(route: &str) -> bool {
    matches!(route, "toc.html" | "print.html" | "404.html")
}

/// Returns the diagnostics for one rendered site, or an empty string when it holds.
pub fn check_rendered(rendered: &Rendered, forbid: &[String]) -> String {
    let root = rendered.root.as_path();
    let pages = pages(root);
    if pages.is_empty() {
        return "rendered site has no HTML page".to_string();
    }
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for page in &pages {
        ids.insert(page.route.clone(), page.ids.clone());
    }

    if !root.join("index.html").is_file() {
        return "rendered site has no index.html at its root".to_string();
    }
    if !root.join("searchindex.js").is_file() {
        return "rendered site has no searchindex.js".to_string();
    }
    if !root.join("css/general.css").is_file() {
        return "rendered site has no stylesheet".to_string();
    }
    if !root.join("book.js").is_file() {
        return "rendered site has no book.js navigation script".to_string();
    }
    if !root.join("print.html").is_file() {
        return "rendered site has no print.html".to_string();
    }

    let search = fs::read_to_string(root.join("searchindex.js")).unwrap_or_default();
    if !search.contains("window.search") {
        return "searchindex.js holds no search object".to_string();
    }

    let mut errors: Vec<String> = Vec::new();
    for page in &pages {
        if !is_fragment(&page.route) {
            if page.title.is_empty() {
                errors.push(format!("{} has no <title>", page.route));
            }
            if page.headings.is_empty() {
                errors.push(format!("{} has no heading", page.route));
            }
            if !page.has_viewport {
                errors.push(format!("{} has no viewport meta tag", page.route));
            }
            if !page.has_nav {
                errors.push(format!("{} has no navigation element", page.route));
            }
        }
        for link in &page.links {
            if !is_internal(link) {
                continue;
            }
            let (route, anchor) = split_target(link);
            if route.is_empty() {
                errors.push(format!("{} links to an empty target", page.route));
                continue;
            }
            let target = resolve(&page.route, &route);
            let name = page_name(root, &target);
            if !root.join(&name).is_file() {
                errors.push(format!("{} links to missing '{link}'", page.route));
                continue;
            }
            if !anchor.is_empty() && !ids.get(&name).is_some_and(|found| found.contains(&anchor)) {
                errors.push(format!("{} links to missing anchor '{link}'", page.route));
            }
        }
        for asset in &page.assets {
            if !is_internal(asset) {
                continue;
            }
            let (route, _) = split_target(asset);
            if route.is_empty() {
                continue;
            }
            let target = resolve(&page.route, &route);
            let name = page_name(root, &target);
            if !root.join(&name).exists() {
                errors.push(format!("{} references missing asset '{asset}'", page.route));
            }
        }
    }

    let index = pages
        .iter()
        .find(|page| page.route == "index.html")
        .cloned()
        .unwrap_or_default();
    if !index.ids.iter().any(|id| id == "search-toggle") {
        errors.push("index.html has no search toggle".to_string());
    }
    if !index.ids.iter().any(|id| id == "sidebar-toggle") {
        errors.push("index.html has no sidebar toggle".to_string());
    }
    if rendered.landing {
        if index.code_blocks == 0 {
            errors.push("index.html renders no code block".to_string());
        }
        if index.highlighted == 0 {
            errors.push("index.html renders no highlighted code block".to_string());
        }
        if index.list_items == 0 {
            errors.push("index.html renders no list".to_string());
        }
    }

    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for page in &pages {
        if seen
            .insert(page.route.as_str(), page.title.as_str())
            .is_some()
        {
            errors.push(format!("{} repeats a route", page.route));
        }
    }

    for needle in forbid {
        if needle.is_empty() {
            continue;
        }
        if let Some(page) = pages.iter().find(|page| {
            fs::read_to_string(root.join(&page.route)).is_ok_and(|html| html.contains(needle))
        }) {
            errors.push(format!("{} publishes {needle:?}", page.route));
        }
    }

    errors.into_iter().collect::<Vec<_>>().join("\n")
}

/// Returns a report naming one route per rendered page, for negative-case fixtures.
pub fn route_report(rendered: &Rendered) -> String {
    let mut text = String::new();
    for page in pages(rendered.root.as_path()) {
        let _ = writeln!(
            text,
            "{}: title={} headings={} code={} highlighted={} lists={} tables={}",
            page.route,
            page.title,
            page.headings.len(),
            page.code_blocks,
            page.highlighted,
            page.list_items,
            page.tables
        );
    }
    text
}

#[cfg(test)]
#[path = "rendered_tests.rs"]
mod rendered_tests;
