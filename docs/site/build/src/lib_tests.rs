use super::{
    alias_errors, alias_route, log_error, manifest_error, manifest_pages, pages_error, parse,
    route_error, run, usage, Options, Page,
};
use std::fs;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("dx-site-build-tests-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a writable scratch directory");
    dir
}

fn write(dir: &PathBuf, name: &str, text: &str) -> String {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("a writable parent");
    }
    fs::write(&path, text).expect("a writable file");
    path.to_string_lossy().into_owned()
}

fn page(route: &str, path: &str) -> Page {
    Page {
        route: route.to_string(),
        path: path.to_string(),
    }
}

#[test]
fn usage_names_every_required_input() {
    let line = usage();
    for flag in ["--mdbook", "--book", "--summary", "--manifest", "--out"] {
        assert!(line.contains(flag), "{flag}: {line}");
    }
}

#[test]
fn parse_requires_every_input_and_names_the_missing_one() {
    let error = parse(&[]).expect_err("no inputs");
    assert_eq!(error, "--mdbook is required");
    let error = parse(&["--book".to_string(), "b".to_string()]).expect_err("a missing mdbook");
    assert_eq!(error, "--mdbook is required");
    assert!(
        parse(&["--out".to_string()]).is_err(),
        "a flag without a value"
    );
}

#[test]
fn parse_rejects_an_unknown_flag() {
    let error = parse(&["--nope".to_string(), "x".to_string()]).expect_err("an unknown flag");
    assert_eq!(error, "unknown argument '--nope'");
}

#[test]
fn routes_must_be_relative_markdown_paths_inside_the_book() {
    let cases = [
        ("README.md", ""),
        ("docs/README.md", ""),
        (
            "guide.txt",
            "docs_site: route 'guide.txt' is not a Markdown page",
        ),
        (
            "/docs/README.md",
            "docs_site: route '/docs/README.md' is not a relative clean path",
        ),
        (
            "docs\\README.md",
            "docs_site: route 'docs\\README.md' is not a relative clean path",
        ),
        (
            "docs/my page.md",
            "docs_site: route 'docs/my page.md' is not a relative clean path",
        ),
        (
            "../escape.md",
            "docs_site: route '../escape.md' has an empty or relative segment",
        ),
        (
            "docs//README.md",
            "docs_site: route 'docs//README.md' has an empty or relative segment",
        ),
        ("", "docs_site: a source route is empty"),
    ];
    for (route, want) in cases {
        assert_eq!(route_error(route), want, "route {route:?}");
    }
}

#[test]
fn page_lists_reject_a_repeated_route_and_a_page_without_a_path() {
    let pages = vec![
        page("docs/README.md", "a.md"),
        page("docs/README.md", "b.md"),
    ];
    assert_eq!(
        pages_error(&pages),
        "docs_site: route 'docs/README.md' is declared twice"
    );
    let pages = vec![page("docs/README.md", "")];
    assert_eq!(
        pages_error(&pages),
        "docs_site: route 'docs/README.md' names no source page"
    );
    let pages = vec![
        page("docs/README.md", "a.md"),
        page("docs/other.md", "b.md"),
    ];
    assert_eq!(pages_error(&pages), "");
}

#[test]
fn readme_pages_get_the_alias_mdbook_own_rewriting_needs() {
    assert_eq!(alias_route("README.md").as_deref(), Some("README.html"));
    assert_eq!(
        alias_route("docs/cli/commands/README.md").as_deref(),
        Some("docs/cli/commands/README.html")
    );
    assert_eq!(alias_route("docs/docs.md"), None);
    assert_eq!(alias_route("guide.txt"), None);
}

#[test]
fn manifest_lines_carry_a_route_and_a_path() {
    let text = "docs/README.md\tdocs/README.md\nREADME.md\tindex.md\n";
    assert_eq!(manifest_error(text), "");
    let pages = manifest_pages(text);
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert_eq!(
        pages[0],
        page("docs/README.md", "docs/README.md"),
        "{pages:?}"
    );
    assert_eq!(
        manifest_error("docs/README.md\n"),
        "docs_site: page line needs a route and a path: docs/README.md"
    );
    assert_eq!(
        manifest_error("docs/README.md\t\n"),
        "docs_site: page line for route 'docs/README.md' names no path"
    );
}

#[test]
fn mdbook_warnings_and_errors_fail_even_when_mdbook_exits_zero() {
    let clean = "2026-10-06 [INFO] (mdbook::book): Book building has started\n";
    assert_eq!(log_error(clean), "");
    assert_eq!(
        log_error("2026-10-06 [WARN] (mdbook::preprocess::links): missing page\n"),
        "mdbook reported [WARN] (mdbook::preprocess::links): missing page"
    );
    assert_eq!(
        log_error("2026-10-06 [ERROR] (mdbook::utils): Error: no SUMMARY.md\n"),
        "mdbook reported [ERROR] (mdbook::utils): Error: no SUMMARY.md"
    );
    assert_eq!(
        log_error("2026-10-06 [INFO] first\n2026-10-06 [WARN] second\n"),
        "mdbook reported [WARN] second"
    );
}

#[test]
fn run_rejects_an_unreadable_manifest() {
    let options = Options {
        mdbook: "mdbook".to_string(),
        book: "book.toml".to_string(),
        summary: "SUMMARY.md".to_string(),
        manifest: "no-such-manifest".to_string(),
        out: "out".to_string(),
    };
    let refusal = run(&options);
    assert!(
        refusal.contains("page manifest 'no-such-manifest' is unreadable"),
        "{refusal}"
    );
}

#[test]
fn run_rejects_a_manifest_that_names_a_page_twice() {
    let dir = scratch("duplicate");
    let manifest = write(
        &dir,
        "pages.tsv",
        "docs/README.md\ta.md\ndocs/README.md\tb.md\n",
    );
    let options = Options {
        mdbook: "mdbook".to_string(),
        book: write(&dir, "book.toml", "[book]\n"),
        summary: write(&dir, "SUMMARY.md", "# Summary\n"),
        manifest,
        out: dir.join("out").to_string_lossy().into_owned(),
    };
    let refusal = run(&options);
    assert_eq!(
        refusal,
        "docs_site: route 'docs/README.md' is declared twice"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_reports_a_missing_mdbook_binary() {
    let dir = scratch("missing-binary");
    let guide = write(&dir, "guide.md", "# Guide\n");
    let manifest = write(&dir, "pages.tsv", &format!("docs/guide.md\t{guide}\n"));
    let options = Options {
        mdbook: dir.join("no-such-mdbook").to_string_lossy().into_owned(),
        book: write(&dir, "book.toml", "[book]\n"),
        summary: write(&dir, "SUMMARY.md", "# Summary\n"),
        manifest,
        out: dir.join("out").to_string_lossy().into_owned(),
    };
    let refusal = run(&options);
    assert!(refusal.starts_with("cannot run mdbook "), "{refusal}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn alias_errors_name_the_rendered_page_mdbook_never_wrote() {
    let dir = scratch("alias");
    let out = dir.join("out");
    fs::create_dir_all(out.join("docs")).expect("a writable output directory");
    let refusal = alias_errors(&[page("docs/README.md", "a.md")], &out);
    assert!(refusal.contains("mdbook rendered no '"), "{refusal}");
    assert!(refusal.contains("index.html"), "{refusal}");
    let _ = fs::remove_dir_all(&dir);
}
