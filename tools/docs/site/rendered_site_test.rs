//! Asserts the rendered user site is a real multi-page documentation website.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const USER_SITE: &str = "DX_USER_SITE";
const DEMO_SITE: &str = "DX_DEMO_SITE";
const SITE_CHECK: &str = "DX_SITE_CHECK";

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn expect_ok(&self) -> &Self {
        assert_eq!(
            self.code, 0,
            "expected the rendered site to pass; stdout {} stderr {}",
            self.stdout, self.stderr
        );
        self
    }

    fn expect_refusal(&self, detail: &str) -> &Self {
        assert_eq!(self.code, 1, "expected a refusal naming {detail:?}");
        assert!(
            self.stderr.contains(detail),
            "stderr must name {detail:?}, got {}",
            self.stderr
        );
        self
    }
}

fn runfiles(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn site(var: &str) -> PathBuf {
    let path = runfiles(var);
    assert!(
        path.is_dir(),
        "{} is not a directory: {}",
        var,
        path.display()
    );
    path
}

fn check(args: &[String]) -> Run {
    let output = Command::new(runfiles(SITE_CHECK))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run site_check: {error}"));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn check_rendered(root: &Path, extra: &[&str]) -> Run {
    let mut args = vec!["--rendered".to_string(), root.display().to_string()];
    args.extend(extra.iter().map(|flag| (*flag).to_string()));
    check(&args)
}

fn read(root: &Path, route: &str) -> String {
    std::fs::read_to_string(root.join(route))
        .unwrap_or_else(|error| panic!("read {}: {error}", route))
}

fn every_html(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("read dir: {error}"));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "html") {
                out.push(
                    path.strip_prefix(root)
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_rendered_user_site_passes_the_rendered_output_check() {
    check_rendered(&site(USER_SITE), &["--landing"]).expect_ok();
}

#[test]
fn the_rendered_demo_site_passes_the_rendered_output_check() {
    check_rendered(&site(DEMO_SITE), &[]).expect_ok();
}

#[test]
fn every_command_page_has_its_own_route() {
    let root = site(USER_SITE);
    let routes = every_html(&root);
    for page in [
        "docs/cli/commands/build-test-coverage.html",
        "docs/cli/commands/quality.html",
        "docs/cli/commands/check-fix-clean.html",
        "docs/cli/commands/generate.html",
        "docs/cli/commands/docs.html",
        "docs/cli/commands/hooks.html",
        "docs/cli/commands/inspect.html",
        "docs/cli/commands/migrate.html",
        "docs/cli/commands/new-upgrade.html",
        "docs/cli/commands/status-version.html",
        "docs/cli/commands/watch.html",
        "docs/cli/commands/completion.html",
        "docs/cli/commands/capabilities.html",
        "docs/cli/commands/verify.html",
        "docs/cli/commands/rerun.html",
        "docs/cli/commands/tests.html",
        "docs/cli/commands/scope-defaults.html",
        "docs/cli/commands/audit-update-bazel.html",
        "docs/cli/commands/environment-codegen-setup.html",
    ] {
        assert!(
            routes.contains(&page.to_string()),
            "no route for {page}: {routes:?}"
        );
    }
}

#[test]
fn the_site_publishes_every_allowed_user_page() {
    let root = site(USER_SITE);
    let routes = every_html(&root);
    for page in [
        "index.html",
        "docs/index.html",
        "docs/github-ci.html",
        "examples/index.html",
        "examples/adopt-rust/index.html",
        "examples/mixed/hello/index.html",
    ] {
        assert!(
            routes.contains(&page.to_string()),
            "no route for {page}: {routes:?}"
        );
    }
    assert!(
        routes.len() > 30,
        "the user site renders {} pages, want a real book",
        routes.len()
    );
}

#[test]
fn markdown_is_rendered_not_dumped() {
    let root = site(USER_SITE);
    let docs_page = read(&root, "docs/cli/commands/docs.html");
    assert!(
        docs_page.contains("<h1 id=\"dx-docs\">"),
        "the docs page must render a heading: {}",
        docs_page.len()
    );
    assert!(
        docs_page.contains("<pre><code class=\"language-"),
        "the docs page must render a highlighted code block"
    );
    assert!(
        docs_page.contains("<li>"),
        "the docs page must render a list"
    );
    assert!(
        !docs_page.contains("```"),
        "the docs page must not show a raw Markdown fence"
    );
    assert!(
        !docs_page.contains("&gt; `dx docs`"),
        "the docs page must not show escaped Markdown backticks"
    );
    assert!(
        !docs_page.contains("rendered by mdBook 0.4.43 fixture"),
        "no fixture marker may be published"
    );
}

#[test]
fn no_agent_instructions_or_demo_api_are_published() {
    let root = site(USER_SITE);
    check_rendered(
        &root,
        &[
            "--forbid",
            "Treat warnings as errors",
            "--forbid",
            "AccountService",
            "--forbid",
            "Python demo package",
        ],
    )
    .expect_ok();
    for route in every_html(&root) {
        let html = read(&root, &route);
        for needle in ["AGENTS.md", "AccountService", "demo.symbols"] {
            assert!(!html.contains(needle), "{route} publishes {needle:?}");
        }
    }
}

#[test]
fn the_site_carries_its_own_styles_scripts_and_fonts() {
    let root = site(USER_SITE);
    for asset in [
        "css/general.css",
        "css/chrome.css",
        "css/variables.css",
        "css/print.css",
        "book.js",
        "toc.js",
        "highlight.js",
        "highlight.css",
        "searcher.js",
        "elasticlunr.min.js",
        "clipboard.min.js",
        "fonts/fonts.css",
        "favicon.svg",
        "print.html",
        "404.html",
        ".nojekyll",
    ] {
        assert!(root.join(asset).is_file(), "missing asset {asset}");
    }
    assert!(
        std::fs::read_dir(root.join("fonts"))
            .expect("fonts dir")
            .count()
            >= 2,
        "the site must ship its font files"
    );
}

#[test]
fn the_search_index_names_every_rendered_page() {
    let root = site(USER_SITE);
    let index = read(&root, "searchindex.js");
    assert!(
        index.contains("window.search"),
        "the search index must define the mdBook search object"
    );
    for route in [
        "index.html",
        "docs/index.html",
        "docs/cli/commands/index.html",
        "docs/cli/commands/docs.html",
        "docs/github-ci.html",
        "examples/index.html",
    ] {
        assert!(
            index.contains(route),
            "the search index never names {route}"
        );
    }
}

#[test]
fn every_page_links_to_every_other_page_it_names() {
    let root = site(USER_SITE);
    let routes = every_html(&root);
    let commands = read(&root, "docs/cli/commands/index.html");
    for page in [
        "docs.html",
        "quality.html",
        "scope-defaults.html",
        "status-version.html#version-skew",
    ] {
        assert!(
            commands.contains(page),
            "the command reference never links {page}"
        );
    }
    let index = read(&root, "index.html");
    for page in [
        "docs/README.html",
        "docs/cli/commands/README.html",
        "examples/README.html",
    ] {
        assert!(index.contains(page), "the landing page never links {page}");
    }
    assert!(
        routes
            .iter()
            .any(|route| route == "docs/cli/commands/index.html"),
        "the command reference needs its own route"
    );
}

#[test]
fn internal_links_stay_relative_so_the_site_serves_from_any_base() {
    let root = site(USER_SITE);
    for route in every_html(&root) {
        let html = read(&root, &route);
        for attribute in ["href", "src"] {
            for start in html.split(&format!("{attribute}=\"")).skip(1) {
                let Some(value) = start.split_once('"').map(|(head, _)| head) else {
                    continue;
                };
                if value.is_empty() || value.starts_with('#') || value.contains("://") {
                    continue;
                }
                assert!(
                    !value.starts_with('/'),
                    "{route} hardcodes a root-relative link {attribute}=\"{value}\""
                );
            }
        }
    }
}

#[test]
fn the_404_page_resolves_its_assets_from_any_base_url() {
    let root = site(USER_SITE);
    let not_found = read(&root, "404.html");
    assert!(
        not_found.contains("<base href=\"./\">"),
        "the 404 page must use a base URL that works at / and /rules_dx/"
    );
    assert!(
        !not_found.contains("ralvik.github.io"),
        "the 404 page must not hardcode a production URL"
    );
}

#[test]
fn command_pages_carry_meaningful_titles_and_headings() {
    let root = site(USER_SITE);
    for route in every_html(&root)
        .into_iter()
        .filter(|route| route != "toc.html")
    {
        let html = read(&root, &route);
        let title = html
            .split_once("<title>")
            .and_then(|(_, tail)| tail.split_once("</title>").map(|(head, _)| head))
            .unwrap_or_default()
            .trim()
            .to_string();
        assert!(!title.is_empty(), "{route} has an empty title");
        assert!(
            title.ends_with("rules_dx"),
            "{route} title must name the book: {title:?}"
        );
    }
    let docs_page = read(&root, "docs/cli/commands/docs.html");
    assert!(
        docs_page.contains("<title>Docs - rules_dx</title>"),
        "the docs page title must name its own page"
    );
}

#[test]
fn the_landing_page_renders_search_and_keyboard_navigation() {
    let root = site(USER_SITE);
    let index = read(&root, "index.html");
    for id in [
        "search-toggle",
        "sidebar",
        "sidebar-toggle",
        "sidebar-toggle-anchor",
        "menu-bar",
        "searchbar",
        "theme-toggle",
    ] {
        assert!(index.contains(&format!("id=\"{id}\"")), "no {id} control");
    }
    let chrome = read(&root, "css/chrome.css");
    for selector in [
        "#searchbar:focus",
        "#sidebar-toggle-anchor:checked ~ .page-wrapper",
        "#sidebar-toggle-anchor:not(:checked) ~ .sidebar",
        "sidebar-visible",
    ] {
        assert!(
            chrome.contains(selector),
            "the stylesheet has no {selector} rule, so focus and toggles are invisible"
        );
    }
    assert!(
        chrome.contains("@media"),
        "the stylesheet has no media query, so the layout is not responsive"
    );
}

#[test]
fn every_page_declares_a_mobile_viewport_and_a_language() {
    let root = site(USER_SITE);
    for route in every_html(&root) {
        let html = read(&root, &route);
        assert!(
            html.contains("name=\"viewport\""),
            "{route} has no viewport meta tag"
        );
        assert!(
            html.contains("width=device-width"),
            "{route} does not adapt to the viewport width"
        );
        assert!(
            html.contains("<html lang=\"en\""),
            "{route} declares no language"
        );
    }
}

#[test]
fn nested_example_pages_render_their_own_route_and_links() {
    let root = site(USER_SITE);
    let page = read(&root, "examples/mixed/hello/index.html");
    assert!(
        page.contains("<h1 id="),
        "the nested example page must render its own heading"
    );
    let examples = read(&root, "examples/index.html");
    for target in ["mixed/hello/", "adopt-rust/", "consumer-ci/", "docs-ci/"] {
        assert!(
            examples.contains(&format!("href=\"{target}")),
            "the examples page must link {target}"
        );
    }
}

#[test]
fn special_characters_in_page_content_render_escaped() {
    let root = site(USER_SITE);
    let api = read(&root, "index.html");
    assert!(
        api.contains("<code>dx</code>"),
        "inline code must render as an element: {api}"
    );
    let commands = read(&root, "docs/cli/commands/index.html");
    assert!(
        commands.contains("<code>dx</code>") || commands.contains("<code>bazel run"),
        "the command reference must render inline code"
    );
    let demo = read(&site(DEMO_SITE), "api.html");
    assert!(
        demo.contains("AccountService.create"),
        "the demo API page must render its symbol names"
    );
}

#[test]
fn a_site_missing_a_route_is_refused_by_the_rendered_check() {
    let scratch = dx_testing::mkscratch("rendered-missing-route")
        .unwrap_or_else(|error| panic!("test scratch: {error}"));
    copy_tree(&site(USER_SITE), &scratch);
    let commands = scratch.join("docs/cli/commands/README.html");
    let body = std::fs::read_to_string(&commands).expect("read the command reference");
    assert!(
        body.contains("docs.html"),
        "the command reference must link its own pages: {}",
        body.len()
    );
    std::fs::remove_file(scratch.join("docs/cli/commands/docs.html"))
        .expect("remove a command page");
    check_rendered(&scratch, &["--landing"]).expect_refusal("links to missing");
}

#[test]
fn a_site_with_a_dangling_anchor_is_refused_by_the_rendered_check() {
    let scratch = dx_testing::mkscratch("rendered-missing-anchor").expect("test scratch");
    copy_tree(&site(USER_SITE), &scratch);
    let index = std::fs::read_to_string(scratch.join("index.html")).expect("read index");
    std::fs::write(
        scratch.join("index.html"),
        index.replace("docs/README.html", "docs/index.html#no-such-anchor"),
    )
    .expect("write index");
    check_rendered(&scratch, &["--landing"]).expect_refusal("links to missing anchor");
}

#[test]
fn a_site_that_demotes_its_navigation_widgets_is_refused() {
    let scratch = dx_testing::mkscratch("rendered-missing-widgets").expect("test scratch");
    copy_tree(&site(USER_SITE), &scratch);
    let index = std::fs::read_to_string(scratch.join("index.html")).expect("read index");
    std::fs::write(
        scratch.join("index.html"),
        index
            .replace("id=\"search-toggle\"", "class=\"gone\"")
            .replace("id=\"sidebar-toggle\"", "class=\"gone\""),
    )
    .expect("write index");
    check_rendered(&scratch, &["--landing"]).expect_refusal("has no search toggle");
}

#[test]
fn a_site_without_its_stylesheet_is_refused() {
    let scratch = dx_testing::mkscratch("rendered-missing-css").expect("test scratch");
    copy_tree(&site(USER_SITE), &scratch);
    std::fs::remove_file(scratch.join("css/general.css")).expect("remove the stylesheet");
    check_rendered(&scratch, &["--landing"]).expect_refusal("has no stylesheet");
}

#[test]
fn a_site_with_a_root_relative_link_is_refused() {
    let scratch = dx_testing::mkscratch("rendered-root-relative").expect("test scratch");
    copy_tree(&site(USER_SITE), &scratch);
    let index = std::fs::read_to_string(scratch.join("index.html")).expect("read index");
    std::fs::write(
        scratch.join("index.html"),
        index.replace(
            "href=\"docs/README.html\"",
            "href=\"/rules_dx/docs/README.html\"",
        ),
    )
    .expect("write index");
    check_rendered(&scratch, &["--landing"])
        .expect_refusal("references missing asset '/rules_dx/docs/README.html'");
}

#[test]
fn the_rendered_output_is_a_function_of_its_sources() {
    let user = site(USER_SITE);
    let demo = site(DEMO_SITE);
    let mut digests: BTreeSet<u64> = BTreeSet::new();
    for root in [&user, &demo] {
        for route in every_html(root) {
            digests.insert(read(root, &route).len() as u64);
        }
    }
    assert!(
        digests.len() > 5,
        "the rendered sites are suspiciously small"
    );
    let repeat = std::fs::read_to_string(user.join("index.html")).expect("read index");
    assert_eq!(
        repeat,
        read(&user, "index.html"),
        "the render is not stable"
    );
}

fn copy_tree(from: &Path, to: &Path) {
    copy_readonly(from, to);
}

fn copy_readonly(from: &Path, to: &Path) {
    for route in every_html(from) {
        let target = to.join(&route);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        write_copy(&from.join(&route), &target);
    }
    for asset in ["css", "js", "fonts", "book.js", "searchindex.js"] {
        let source = from.join(asset);
        if !source.exists() {
            continue;
        }
        if source.is_dir() {
            copy_dir(&source, &to.join(asset));
        } else {
            write_copy(&source, &to.join(asset));
        }
    }
    for page in ["print.html", "404.html", "toc.html"] {
        if from.join(page).is_file() {
            write_copy(&from.join(page), &to.join(page));
        }
    }
}

fn write_copy(from: &Path, to: &Path) {
    let body =
        std::fs::read(from).unwrap_or_else(|error| panic!("read {}: {error}", from.display()));
    std::fs::write(to, body).unwrap_or_else(|error| panic!("write {}: {error}", to.display()));
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create dir");
    let entries = std::fs::read_dir(from).expect("read dir");
    for entry in entries.flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            write_copy(&path, &target);
        }
    }
}
