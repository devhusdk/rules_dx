use super::{
    ArgsError, BuildError, Options, Page, alias_route, api_pages, check_log, check_manifest,
    check_pages, check_route, manifest_pages, parse, run, stage, usage, write_aliases,
};
use dx_atomic_fs::path_io::PathIoError;
use std::fs;
use std::path::Path;

fn message(result: Result<(), BuildError>) -> String {
    result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
}

fn write(dir: &Path, name: &str, text: &str) -> String {
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
    assert_eq!(error.to_string(), "--mdbook is required");
    let error = parse(&["--book".to_string(), "b".to_string()]).expect_err("a missing mdbook");
    assert_eq!(
        error,
        ArgsError::Required {
            flag: "--mdbook".to_owned()
        }
    );
    let error = parse(&["--out".to_string()]).expect_err("a flag without a value");
    assert_eq!(error.to_string(), "--out needs a value");
}

#[test]
fn parse_rejects_an_unknown_flag() {
    let error = parse(&["--nope".to_string(), "x".to_string()]).expect_err("an unknown flag");
    assert_eq!(error.to_string(), "unknown argument '--nope'");
    assert!(matches!(error, ArgsError::Unknown { .. }), "{error:?}");
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
        assert_eq!(message(check_route(route)), want, "route {route:?}");
    }
}

#[test]
fn page_lists_reject_a_repeated_route_and_a_page_without_a_path() {
    let pages = vec![
        page("docs/README.md", "a.md"),
        page("docs/README.md", "b.md"),
    ];
    let error = check_pages(&pages).expect_err("a repeated route");
    assert_eq!(
        error.to_string(),
        "docs_site: route 'docs/README.md' is declared twice"
    );
    assert!(
        matches!(error, BuildError::DuplicateRoute { .. }),
        "{error:?}"
    );
    let pages = vec![page("docs/README.md", "")];
    let error = check_pages(&pages).expect_err("a page without a path");
    assert_eq!(
        error.to_string(),
        "docs_site: route 'docs/README.md' names no source page"
    );
    assert!(matches!(error, BuildError::NoSource { .. }), "{error:?}");
    let pages = vec![
        page("docs/README.md", "a.md"),
        page("docs/other.md", "b.md"),
    ];
    check_pages(&pages).expect("distinct routes");
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
    assert_eq!(alias_route("docs/MY_README.md"), None);
    assert_eq!(alias_route("MY_README.md"), None);
}

#[test]
fn write_aliases_leaves_a_non_readme_page_alone() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-alias-other").unwrap();
    let dir = scratch.path().to_path_buf();
    let out = dir.join("out");
    fs::create_dir_all(out.join("docs")).expect("a writable output directory");
    fs::write(out.join("docs/MY_README.html"), "kept").expect("a writable page");
    write_aliases(&[page("docs/MY_README.md", "a.md")], &out).expect("no alias needed");
    let kept = fs::read_to_string(out.join("docs/MY_README.html")).expect("the page");
    assert_eq!(kept, "kept");
}

#[test]
fn manifest_lines_carry_a_route_and_a_path() {
    let text = "docs/README.md\tdocs/README.md\nREADME.md\tindex.md\n";
    check_manifest(text).expect("a valid manifest");
    let pages = manifest_pages(text);
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert_eq!(
        pages[0],
        page("docs/README.md", "docs/README.md"),
        "{pages:?}"
    );
    let error = check_manifest("docs/README.md\n").expect_err("a line without a path");
    assert_eq!(
        error.to_string(),
        "docs_site: page line needs a route and a path: docs/README.md"
    );
    assert!(
        matches!(error, BuildError::ManifestLine { .. }),
        "{error:?}"
    );
    let error = check_manifest("docs/README.md\t\n").expect_err("an empty path");
    assert_eq!(
        error.to_string(),
        "docs_site: page line for route 'docs/README.md' names no path"
    );
    assert!(
        matches!(error, BuildError::ManifestPath { .. }),
        "{error:?}"
    );
}

#[test]
fn mdbook_warnings_and_errors_fail_even_when_mdbook_exits_zero() {
    let clean = "2026-10-06 [INFO] (mdbook::book): Book building has started\n";
    check_log(clean).expect("a clean log");
    assert_eq!(
        message(check_log(
            "2026-10-06 [WARN] (mdbook::preprocess::links): missing page\n"
        )),
        "mdbook reported [WARN] (mdbook::preprocess::links): missing page"
    );
    assert_eq!(
        message(check_log(
            "2026-10-06 [ERROR] (mdbook::utils): Error: no SUMMARY.md\n"
        )),
        "mdbook reported [ERROR] (mdbook::utils): Error: no SUMMARY.md"
    );
    assert_eq!(
        message(check_log(
            "2026-10-06 [INFO] first\n2026-10-06 [WARN] second\n"
        )),
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
        ..Options::default()
    };
    let error = run(&options).expect_err("an unreadable manifest");
    assert!(
        matches!(&error, BuildError::Io(PathIoError::Read { path, .. }) if path == Path::new("no-such-manifest")),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .starts_with("docs_site: cannot read no-such-manifest: "),
        "{error}"
    );
}

#[test]
fn run_rejects_an_empty_manifest() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-empty-manifest").unwrap();
    let dir = scratch.path().to_path_buf();
    let manifest = write(&dir, "pages.tsv", "");
    let options = Options {
        mdbook: "mdbook".to_string(),
        book: "book.toml".to_string(),
        summary: "SUMMARY.md".to_string(),
        manifest: manifest.clone(),
        out: "out".to_string(),
        ..Options::default()
    };
    let error = run(&options).expect_err("an empty manifest");
    assert_eq!(
        error.to_string(),
        format!("docs_site: page manifest '{manifest}' holds no page")
    );
    assert!(
        matches!(error, BuildError::EmptyManifest { .. }),
        "{error:?}"
    );
}

#[test]
fn run_names_a_missing_summary() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-missing-summary").unwrap();
    let dir = scratch.path().to_path_buf();
    let guide = write(&dir, "guide.md", "# Guide\n");
    let manifest = write(&dir, "pages.tsv", &format!("docs/guide.md\t{guide}\n"));
    let summary = dir.join("no-such-summary.md");
    let options = Options {
        mdbook: "mdbook".to_string(),
        book: write(&dir, "book.toml", "[book]\n"),
        summary: summary.to_string_lossy().into_owned(),
        manifest,
        out: dir.join("out").to_string_lossy().into_owned(),
        ..Options::default()
    };
    let error = run(&options).expect_err("a missing summary");
    assert!(
        matches!(&error, BuildError::Io(PathIoError::Read { path, .. }) if *path == summary),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .starts_with(&format!("docs_site: cannot read {}: ", summary.display())),
        "{error}"
    );
}

#[test]
fn run_rejects_a_manifest_that_names_a_page_twice() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-duplicate").unwrap();
    let dir = scratch.path().to_path_buf();
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
        ..Options::default()
    };
    assert_eq!(
        message(run(&options)),
        "docs_site: route 'docs/README.md' is declared twice"
    );
}

#[test]
fn run_reports_a_missing_mdbook_binary() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-missing-binary").unwrap();
    let dir = scratch.path().to_path_buf();
    let guide = write(&dir, "guide.md", "# Guide\n");
    let manifest = write(&dir, "pages.tsv", &format!("docs/guide.md\t{guide}\n"));
    let options = Options {
        mdbook: dir.join("no-such-mdbook").to_string_lossy().into_owned(),
        book: write(&dir, "book.toml", "[book]\n"),
        summary: write(&dir, "SUMMARY.md", "# Summary\n"),
        manifest,
        out: dir.join("out").to_string_lossy().into_owned(),
        ..Options::default()
    };
    let error = run(&options).expect_err("a missing mdbook");
    assert!(matches!(error, BuildError::RunMdbook { .. }), "{error:?}");
    assert!(
        error.to_string().starts_with("cannot run mdbook "),
        "{error}"
    );
}

#[test]
fn write_aliases_names_the_rendered_page_mdbook_never_wrote() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-alias").unwrap();
    let dir = scratch.path().to_path_buf();
    let out = dir.join("out");
    fs::create_dir_all(out.join("docs")).expect("a writable output directory");
    let refusal = message(write_aliases(&[page("docs/README.md", "a.md")], &out));
    assert!(refusal.contains("mdbook rendered no '"), "{refusal}");
    assert!(refusal.contains("index.html"), "{refusal}");
}

#[test]
fn parse_takes_the_api_tree_and_summary_together() {
    let base: Vec<String> = [
        "--mdbook",
        "m",
        "--book",
        "b",
        "--summary",
        "s",
        "--manifest",
        "p",
        "--out",
        "o",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let mut both = base.clone();
    both.extend(["--api-tree", "t", "--api-summary", "f"].map(str::to_string));
    let options = parse(&both).expect("both api inputs");
    assert_eq!(
        (options.api_tree.as_str(), options.api_summary.as_str()),
        ("t", "f")
    );
    let mut tree_only = base.clone();
    tree_only.extend(["--api-tree", "t"].map(str::to_string));
    let error = parse(&tree_only).expect_err("a tree without a summary");
    assert_eq!(error, ArgsError::ApiPair);
    assert_eq!(
        error.to_string(),
        "--api-tree and --api-summary go together"
    );
    assert!(parse(&base).is_ok(), "no API inputs at all");
}

#[test]
fn an_api_tree_routes_every_page_under_api() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-api-tree").unwrap();
    let dir = scratch.path().to_path_buf();
    let a = write(&dir, "tree/rust/demo/add.md", "# add\n");
    let b = write(&dir, "tree/rust/demo/shapes.Circle.md", "# Circle\n");
    let pages = api_pages(&dir.join("tree")).expect("pages");
    assert_eq!(
        pages,
        vec![
            page("api/rust/demo/add.md", &a),
            page("api/rust/demo/shapes.Circle.md", &b),
        ]
    );
    let mut with_manifest = vec![page("api/rust/demo/add.md", "other.md")];
    with_manifest.extend(pages);
    assert_eq!(
        message(check_pages(&with_manifest)),
        "docs_site: route 'api/rust/demo/add.md' is declared twice"
    );
    let _: String = write(&dir, "tree/rust/demo/bad name.md", "# bad\n");
    let bad = api_pages(&dir.join("tree")).expect("pages");
    assert_eq!(
        message(check_pages(&bad)),
        "docs_site: route 'api/rust/demo/bad name.md' is not a relative clean path"
    );
    fs::create_dir_all(dir.join("empty")).expect("empty tree");
    let error = api_pages(&dir.join("empty")).expect_err("empty tree");
    assert!(
        matches!(error, BuildError::EmptyApiTree { .. }),
        "{error:?}"
    );
    assert!(error.to_string().ends_with("holds no page"), "{error}");
}

#[test]
fn run_rejects_an_api_page_that_collides_with_a_manifest_route() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-api-collision").unwrap();
    let dir = scratch.path().to_path_buf();
    let page_path = write(&dir, "add.md", "# add\n");
    let manifest = write(
        &dir,
        "pages.tsv",
        &format!("api/rust/demo/add.md\t{page_path}\n"),
    );
    let _: String = write(&dir, "tree/rust/demo/add.md", "# add\n");
    let options = Options {
        mdbook: "mdbook".to_string(),
        book: write(&dir, "book.toml", "[book]\n"),
        summary: write(&dir, "SUMMARY.md", "# Summary\n"),
        manifest,
        out: dir.join("out").to_string_lossy().into_owned(),
        api_tree: dir.join("tree").to_string_lossy().into_owned(),
        api_summary: write(&dir, "api_summary.md", "- [API](api.md)\n"),
    };
    assert_eq!(
        message(run(&options)),
        "docs_site: route 'api/rust/demo/add.md' is declared twice"
    );
}

#[test]
fn stage_appends_the_api_summary_to_a_read_only_summary() {
    let scratch = dx_test_scratch::scratch("dx-site-build-tests-read-only-summary").unwrap();
    let dir = scratch.path().to_path_buf();
    let summary = write(&dir, "SUMMARY.md", "# Summary");
    let mut readonly = fs::metadata(&summary).expect("a summary").permissions();
    readonly.set_readonly(true);
    fs::set_permissions(&summary, readonly).expect("a read-only summary");
    let options = Options {
        book: write(&dir, "book.toml", "[book]\n"),
        summary,
        api_summary: write(&dir, "api_summary.md", "- [API](api.md)\n"),
        ..Options::default()
    };
    let book = dir.join("book");
    stage(&book, &options, &[]).expect("a staged book");
    assert_eq!(
        fs::read_to_string(book.join("src/SUMMARY.md")).expect("a staged summary"),
        "# Summary\n- [API](api.md)\n"
    );
}
