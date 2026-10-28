//! Tests for the rendered-site inspector.

use std::fs;
use std::path::Path;

use super::{
    check_rendered, decode_fragment, is_internal, pages, resolve, route_report, split_target,
    usage, Rendered,
};

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

fn site(name: &str) -> dx_test_scratch::TempDir {
    let guard = dx_test_scratch::scratch(name);
    let dir = guard.path();
    write(dir, "index.html", HOME);
    write(dir, "page.html", PAGE);
    write(dir, "nested/deep.html", DEEP);
    write(dir, "print.html", HOME);
    write(dir, "css/general.css", "body {}\n");
    write(dir, "book.js", "// nav\n");
    write(
        dir,
        "searchindex.js",
        "window.search = {\"doc_urls\":[\"index.html#rules_dx\"]};\n",
    );
    guard
}

fn rendered(dir: &Path) -> Rendered {
    Rendered {
        root: dir.to_path_buf(),
        landing: true,
    }
}

#[test]
fn a_complete_rendered_site_holds_together() {
    let guard = site("rendered-ok");
    let dir = guard.path();
    assert_eq!(
        check_rendered(&rendered(dir), &[]),
        "",
        "{}",
        route_report(&rendered(dir))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn every_page_is_found_and_parsed() {
    let guard = site("rendered-pages");
    let dir = guard.path();
    let found = pages(dir);
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
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_missing_asset_fails() {
    let guard = site("rendered-asset");
    let dir = guard.path();
    fs::remove_file(dir.join("book.js")).expect("remove the navigation script");
    let report = check_rendered(&rendered(dir), &[]);
    assert!(
        report.contains("has no book.js navigation script"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_missing_search_index_fails() {
    let guard = site("rendered-search");
    let dir = guard.path();
    fs::remove_file(dir.join("searchindex.js")).expect("remove the search index");
    let report = check_rendered(&rendered(dir), &[]);
    assert!(report.contains("has no searchindex.js"), "{report}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_dangling_link_fails_with_its_target() {
    let guard = site("rendered-link");
    let dir = guard.path();
    write(
        dir,
        "page.html",
        &PAGE.replace("href=\"index.html\"", "href=\"gone.html\""),
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(report.contains("links to missing 'gone.html'"), "{report}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_dangling_anchor_fails_with_its_target() {
    let guard = site("rendered-anchor");
    let dir = guard.path();
    write(
        dir,
        "page.html",
        &PAGE.replace(
            "href=\"nested/deep.html#usage\"",
            "href=\"nested/deep.html#nope\"",
        ),
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(
        report.contains("links to missing anchor 'nested/deep.html#nope'"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

fn with_link(link: &str) -> String {
    PAGE.replace("<p>", &format!("<p><a href=\"{link}\">x</a> "))
}

fn report_for(name: &str, page: &str) -> String {
    let guard = site(name);
    let dir = guard.path();
    write(dir, "page.html", page);
    let report = check_rendered(&rendered(dir), &[]);
    let _ = fs::remove_dir_all(dir);
    report
}

#[test]
fn a_dangling_same_page_anchor_fails_like_a_cross_page_one() {
    let bare = report_for("rendered-same", &with_link("#missing-heading"));
    assert!(
        bare.contains("page.html links to missing anchor '#missing-heading'"),
        "{bare}"
    );
    let full = report_for(
        "rendered-same-full",
        &with_link("page.html#missing-heading"),
    );
    assert!(full.contains("links to missing anchor"), "{full}");
}

#[test]
fn present_empty_and_query_fragments_hold() {
    for link in [
        "#usage",
        "#",
        "#top",
        "page.html#usage",
        "page.html?x=1#usage",
        "?x=1#usage",
    ] {
        assert_eq!(
            report_for("rendered-frag-ok", &with_link(link)),
            "",
            "{link}"
        );
    }
    let bad = report_for("rendered-query-bad", &with_link("page.html?x=1#nope"));
    assert!(bad.contains("links to missing anchor"), "{bad}");
}

#[test]
fn encoded_fragments_match_decoded_ids() {
    let page = with_link("#caf%C3%A9")
        .replace("id=\"usage\"", "id=\"café\"")
        .replace("\"#usage\"", "\"#café\"");
    assert_eq!(report_for("rendered-enc", &page), "");
    let plus = with_link("#a+b")
        .replace("id=\"usage\"", "id=\"a b\"")
        .replace("\"#usage\"", "\"#a b\"");
    assert!(report_for("rendered-plus", &plus).contains("missing anchor"));
    let bad = report_for("rendered-malformed", &with_link("#bad%zz"));
    assert!(bad.contains("malformed anchor encoding '#bad%zz'"), "{bad}");
}

#[test]
fn fragments_percent_decode_without_plus_conversion() {
    assert_eq!(decode_fragment("a%20b+c").as_deref(), Some("a b+c"));
    assert_eq!(decode_fragment("%"), None);
    assert_eq!(decode_fragment("%FF"), None);
}

#[test]
fn a_missing_reference_asset_fails() {
    let guard = site("rendered-css");
    let dir = guard.path();
    write(
        dir,
        "page.html",
        &PAGE.replace("href=\"../css/general.css\"", "href=\"../css/absent.css\""),
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(
        report.contains("references missing asset '../css/absent.css'"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_page_without_a_title_or_heading_fails() {
    let guard = site("rendered-title");
    let dir = guard.path();
    write(
        dir,
        "page.html",
        r#"<!DOCTYPE html><html lang="en"><head></head><body><nav></nav><main><p>x</p></main></body></html>"#,
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(report.contains("page.html has no <title>"), "{report}");
    assert!(report.contains("page.html has no heading"), "{report}");
    assert!(
        report.contains("page.html has no viewport meta tag"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_empty_site_fails() {
    let guard = dx_test_scratch::scratch("rendered-empty");
    let dir = guard.path();
    let report = check_rendered(&rendered(dir), &[]);
    assert_eq!(report, "rendered site has no HTML page");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_site_without_an_index_page_fails() {
    let guard = site("rendered-index");
    let dir = guard.path();
    fs::remove_file(dir.join("index.html")).expect("remove the index page");
    let report = check_rendered(&rendered(dir), &[]);
    assert!(report.contains("has no index.html at its root"), "{report}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_forbidden_needle_is_reported_with_its_page() {
    let guard = site("rendered-forbid");
    let dir = guard.path();
    let report = check_rendered(&rendered(dir), &["sidebar-toggle-anchor".to_string()]);
    assert!(
        report.contains("publishes \"sidebar-toggle-anchor\""),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_landing_page_without_navigation_widgets_fails() {
    let guard = site("rendered-widgets");
    let dir = guard.path();
    write(
        dir,
        "index.html",
        &HOME
            .replace("id=\"sidebar-toggle-anchor\"", "class=\"gone\"")
            .replace("id=\"sidebar-toggle\"", "class=\"gone\"")
            .replace("id=\"search-toggle\"", "class=\"gone\""),
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(
        report.contains("index.html has no search toggle"),
        "{report}"
    );
    assert!(
        report.contains("index.html has no sidebar toggle"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_landing_page_without_a_code_block_fails() {
    let guard = site("rendered-code");
    let dir = guard.path();
    write(
        dir,
        "index.html",
        &HOME.replace(
            "<pre><code class=\"language-sh\">dx --help\n</code></pre>",
            "",
        ),
    );
    let report = check_rendered(&rendered(dir), &[]);
    assert!(
        report.contains("index.html renders no code block"),
        "{report}"
    );
    assert!(
        report.contains("index.html renders no highlighted code block"),
        "{report}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_empty_search_object_fails() {
    let guard = site("rendered-search-object");
    let dir = guard.path();
    write(dir, "searchindex.js", "var search = 1;\n");
    let report = check_rendered(&rendered(dir), &[]);
    assert_eq!(report, "searchindex.js holds no search object");
    let _ = fs::remove_dir_all(dir);
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
