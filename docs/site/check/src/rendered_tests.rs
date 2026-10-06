//! Tests for the rendered-site inspector.

use std::fs;
use std::path::{Path, PathBuf};

use super::{
    check_rendered, is_internal, pages, resolve, route_report, split_target, usage, Rendered,
};

fn scratch(name: &str) -> PathBuf {
    dx_testing::mkscratch(name).unwrap_or_else(|error| panic!("test scratch: {error}"))
}

fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, body).expect("write");
}

const PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<title>Page - rules_dx</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="../css/general.css">
</head>
<body>
<nav id="sidebar"></nav>
<main>
<h1 id="page"><a class="header" href="#page">Page</a></h1>
<h2 id="usage"><a class="header" href="#usage">Usage</a></h2>
<ul><li><a href="index.html">Home</a></li></ul>
<table><tr><td>cell</td></tr></table>
<pre><code class="language-sh">bazel build //...
</code></pre>
<p><a href="nested/deep.html#usage">deep</a> <a href="https://example.com">out</a></p>
</main>
<script src="../book.js"></script>
</body>
</html>
"##;

const HOME: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<title>rules_dx</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
<nav id="sidebar"></nav>
<a href="#" id="sidebar-toggle-anchor"></a>
<a href="#" id="sidebar-toggle" class="sidebar-toggle"></a>
<a href="#searchbar" id="search-toggle" class="search-toggle"></a>
<main>
<h1 id="rules_dx"><a class="header" href="#rules_dx">rules_dx</a></h1>
<ul><li><a href="page.html">Page</a></li></ul>
<pre><code class="language-sh">dx --help
</code></pre>
</main>
<script src="book.js"></script>
</body>
</html>
"##;

const DEEP: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<title>Deep - rules_dx</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="../../css/general.css">
</head>
<body>
<nav id="sidebar"></nav>
<main>
<h1 id="deep"><a class="header" href="#deep">Deep</a></h1>
<h2 id="usage"><a class="header" href="#usage">Usage</a></h2>
<ul><li><a href="../index.html">Home</a></li></ul>
</main>
<script src="../../book.js"></script>
</body>
</html>
"##;

fn site(name: &str) -> PathBuf {
    let dir = scratch(name);
    write(&dir, "index.html", HOME);
    write(&dir, "page.html", PAGE);
    write(&dir, "nested/deep.html", DEEP);
    write(&dir, "print.html", HOME);
    write(&dir, "css/general.css", "body {}\n");
    write(&dir, "book.js", "// nav\n");
    write(
        &dir,
        "searchindex.js",
        "window.search = {\"doc_urls\":[\"index.html#rules_dx\"]};\n",
    );
    dir
}

fn rendered(dir: &Path) -> Rendered {
    Rendered {
        root: dir.to_path_buf(),
        landing: true,
    }
}

#[test]
fn a_complete_rendered_site_holds_together() {
    let dir = site("rendered-ok");
    assert_eq!(
        check_rendered(&rendered(&dir), &[]),
        "",
        "{}",
        route_report(&rendered(&dir))
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn every_page_is_found_and_parsed() {
    let dir = site("rendered-pages");
    let found = pages(&dir);
    let routes: Vec<&str> = found.iter().map(|page| page.route.as_str()).collect();
    assert_eq!(
        routes,
        vec!["index.html", "nested/deep.html", "page.html", "print.html"]
    );
    let page = found.iter().find(|page| page.route == "page.html").unwrap();
    assert_eq!(page.title, "Page - rules_dx", "{page:?}");
    assert!(page.has_viewport, "{page:?}");
    assert!(page.has_nav, "{page:?}");
    assert_eq!(page.list_items, 1, "{page:?}");
    assert_eq!(page.tables, 1, "{page:?}");
    assert_eq!(page.code_blocks, 1, "{page:?}");
    assert_eq!(page.highlighted, 1, "{page:?}");
    assert_eq!(page.headings, vec!["Page", "Usage"], "{page:?}");
    assert!(page.ids.contains(&"usage".to_string()), "{page:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_asset_fails() {
    let dir = site("rendered-asset");
    fs::remove_file(dir.join("book.js")).expect("remove the navigation script");
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(
        report.contains("has no book.js navigation script"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_search_index_fails() {
    let dir = site("rendered-search");
    fs::remove_file(dir.join("searchindex.js")).expect("remove the search index");
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(report.contains("has no searchindex.js"), "{report}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_dangling_link_fails_with_its_target() {
    let dir = site("rendered-link");
    write(
        &dir,
        "page.html",
        &PAGE.replace("href=\"index.html\"", "href=\"gone.html\""),
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(report.contains("links to missing 'gone.html'"), "{report}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_dangling_anchor_fails_with_its_target() {
    let dir = site("rendered-anchor");
    write(
        &dir,
        "page.html",
        &PAGE.replace(
            "href=\"nested/deep.html#usage\"",
            "href=\"nested/deep.html#nope\"",
        ),
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(
        report.contains("links to missing anchor 'nested/deep.html#nope'"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_reference_asset_fails() {
    let dir = site("rendered-css");
    write(
        &dir,
        "page.html",
        &PAGE.replace("href=\"../css/general.css\"", "href=\"../css/absent.css\""),
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(
        report.contains("references missing asset '../css/absent.css'"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_page_without_a_title_or_heading_fails() {
    let dir = site("rendered-title");
    write(
        &dir,
        "page.html",
        r#"<!DOCTYPE html><html lang="en"><head></head><body><nav></nav><main><p>x</p></main></body></html>"#,
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(report.contains("page.html has no <title>"), "{report}");
    assert!(report.contains("page.html has no heading"), "{report}");
    assert!(
        report.contains("page.html has no viewport meta tag"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_site_fails() {
    let dir = scratch("rendered-empty");
    let report = check_rendered(&rendered(&dir), &[]);
    assert_eq!(report, "rendered site has no HTML page");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_site_without_an_index_page_fails() {
    let dir = site("rendered-index");
    fs::remove_file(dir.join("index.html")).expect("remove the index page");
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(report.contains("has no index.html at its root"), "{report}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_forbidden_needle_is_reported_with_its_page() {
    let dir = site("rendered-forbid");
    let report = check_rendered(&rendered(&dir), &["sidebar-toggle-anchor".to_string()]);
    assert!(
        report.contains("publishes \"sidebar-toggle-anchor\""),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_landing_page_without_navigation_widgets_fails() {
    let dir = site("rendered-widgets");
    write(
        &dir,
        "index.html",
        &HOME
            .replace("id=\"sidebar-toggle-anchor\"", "class=\"gone\"")
            .replace("id=\"sidebar-toggle\"", "class=\"gone\"")
            .replace("id=\"search-toggle\"", "class=\"gone\""),
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(
        report.contains("index.html has no search toggle"),
        "{report}"
    );
    assert!(
        report.contains("index.html has no sidebar toggle"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_landing_page_without_a_code_block_fails() {
    let dir = site("rendered-code");
    write(
        &dir,
        "index.html",
        &HOME.replace(
            "<pre><code class=\"language-sh\">dx --help\n</code></pre>",
            "",
        ),
    );
    let report = check_rendered(&rendered(&dir), &[]);
    assert!(
        report.contains("index.html renders no code block"),
        "{report}"
    );
    assert!(
        report.contains("index.html renders no highlighted code block"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_search_object_fails() {
    let dir = site("rendered-search-object");
    write(&dir, "searchindex.js", "var search = 1;\n");
    let report = check_rendered(&rendered(&dir), &[]);
    assert_eq!(report, "searchindex.js holds no search object");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_usage_line_names_every_rendered_flag() {
    for flag in ["--rendered", "--landing", "--forbid"] {
        assert!(usage().contains(flag), "{flag}: {}", usage());
    }
}

#[test]
fn link_targets_split_into_a_route_and_an_anchor() {
    assert_eq!(
        split_target("docs/a.html#usage"),
        ("docs/a.html".into(), "usage".into())
    );
    assert_eq!(
        split_target("docs/a.html"),
        ("docs/a.html".into(), String::new())
    );
    assert_eq!(split_target("#usage"), (String::new(), "usage".into()));
}

#[test]
fn only_relative_targets_stay_inside_the_site() {
    for (target, inside) in [
        ("docs/a.html", true),
        ("#usage", false),
        ("https://example.com", false),
        ("//example.com", false),
        ("mailto:docs@example.com", false),
        ("", false),
    ] {
        assert_eq!(is_internal(target), inside, "{target}");
    }
}

#[test]
fn relative_targets_resolve_against_their_page() {
    assert_eq!(resolve("index.html", "docs/a.html"), "docs/a.html");
    assert_eq!(resolve("docs/index.html", "a.html"), "docs/a.html");
    assert_eq!(resolve("docs/cli/index.html", "../a.html"), "docs/a.html");
    assert_eq!(resolve("docs/index.html", "../../a.html"), "a.html");
    assert_eq!(resolve("docs/index.html", "./a.html"), "docs/a.html");
}
