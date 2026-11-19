//! Tests for the site checker helpers.

use std::path::Path;

use documentation_ir::encode_shard;
use documentation_ir::proto::{DocIr, Symbol};

use super::{
    CheckError, Inputs, anchor_name, check, first_field, has_empty_link_target, inline_targets,
    link_targets, parse_args, resolve_route, split_reference, tidy,
};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(path, body).expect("write");
}

fn write_shard(dir: &Path, ids: &[&str]) {
    let shard = DocIr {
        language: "rust".to_string(),
        package: "demo".to_string(),
        symbols: ids
            .iter()
            .map(|id| Symbol {
                id: id.to_string(),
                ..Symbol::default()
            })
            .collect(),
    };
    let bytes = encode_shard(&shard).expect("encode");
    std::fs::write(dir.join("shard.ir.pb"), bytes).expect("write");
}

fn inputs(dir: &Path) -> Inputs {
    let named = |name: &str| dir.join(name).to_string_lossy().into_owned();
    Inputs {
        book: Some(named("book.toml")),
        api: Some(named("api.md")),
        shards: vec![named("shard.ir.pb")],
        prose: vec![named("guide.md")],
        data: vec![],
        records: None,
    }
}

#[test]
fn an_anchor_matches_mdbook_ids() {
    assert_eq!(anchor_name("## Getting Started"), "getting-started");
    assert_eq!(anchor_name("# A/B and C"), "ab-and-c");
    assert_eq!(anchor_name("### Spaced -- Out"), "spaced----out");
    assert_eq!(anchor_name("#  Trim  Me  "), "trim--me");
    assert_eq!(anchor_name("## rust_library"), "rust_library");
    assert_eq!(anchor_name("## `--affected`"), "--affected");
    assert_eq!(anchor_name("## 中文 Straße"), "中文-straße");
    assert_eq!(anchor_name("## Ünïcode"), "Ünïcode");
}

#[test]
fn an_empty_link_target_is_recognised() {
    assert!(has_empty_link_target("See [gone]()."));
    assert!(!has_empty_link_target("See [gone](here.md)."));
    assert!(!has_empty_link_target("a ] b ( c )"));
}

#[test]
fn every_inline_target_on_a_line_is_found() {
    let found = inline_targets("[a](one.md) and [b](two.md)");
    assert_eq!(found, vec!["one.md".to_string(), "two.md".to_string()]);
}

#[test]
fn a_reference_definition_yields_its_target() {
    let line = "[symbol]: api/python/demo/Thing.create.md";
    assert_eq!(
        split_reference(line).map(|(_, rest)| rest.trim_start()),
        Some("api/python/demo/Thing.create.md")
    );
    assert_eq!(split_reference("not a definition"), None);
}

#[test]
fn titles_are_tidied_the_way_the_checker_expects() {
    assert_eq!(tidy("  <api.md>  "), "api.md");
    assert_eq!(tidy("\"quoted\""), "");
    assert_eq!(tidy("a b"), "a b");
}

#[test]
fn only_the_first_space_delimited_field_survives() {
    assert_eq!(first_field("api.md \"title here\""), "api.md");
    assert_eq!(first_field(""), "");
}

#[test]
fn routes_resolve_against_the_directory_of_the_page() {
    assert_eq!(resolve_route("a.md", "b.md").as_deref(), Some("b.md"));
    assert_eq!(
        resolve_route("a.md", "./cli/x.md").as_deref(),
        Some("cli/x.md")
    );
    assert_eq!(
        resolve_route("cli/a.md", "../b.md").as_deref(),
        Some("b.md")
    );
    assert_eq!(
        resolve_route("cli/a.md", "x.md").as_deref(),
        Some("cli/x.md")
    );
    assert_eq!(resolve_route("a.md", "../b.md"), None);
    assert_eq!(resolve_route("cli/a.md", "../../b.md"), None);
}

#[test]
fn the_required_arguments_are_checked_in_order() {
    assert_eq!(
        parse_args(&args(&["--nope", "x"])).unwrap_err().0,
        "unknown argument '--nope'"
    );
    assert_eq!(
        parse_args(&args(&["--book"])).unwrap_err().0,
        "--book needs a FILE"
    );
    assert_eq!(
        parse_args(&args(&["--api", "api.md"])).unwrap_err().0,
        "--book FILE is required"
    );
    assert_eq!(
        parse_args(&args(&["--book", "b", "--api", "a"]))
            .unwrap_err()
            .0,
        "--shard FILE is required"
    );
    assert_eq!(
        parse_args(&args(&["--book", "b", "--api", "a", "--shard", "s"]))
            .unwrap_err()
            .0,
        "--prose FILE is required"
    );
}

#[test]
fn a_shard_without_symbols_names_no_symbols() {
    let guard = dx_test_scratch::scratch("empty-ids").unwrap();
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(dir, "api.md", "# API\n");
    write_shard(dir, &[]);
    write(dir, "guide.md", "# Guide\n");
    let error = check(&inputs(dir)).unwrap_err();
    assert!(matches!(error, CheckError::NoSymbols), "{error:?}");
    assert_eq!(error.to_string(), "shard names no symbols");
}

#[test]
fn a_shard_that_does_not_decode_is_refused() {
    let guard = dx_test_scratch::scratch("bad-shard").unwrap();
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(dir, "api.md", "# API\n");
    write(dir, "shard.ir.pb", "not a shard\n");
    write(dir, "guide.md", "# Guide\n");
    let error = check(&inputs(dir)).unwrap_err();
    assert!(
        matches!(&error, CheckError::InvalidShard { path, .. } if path.ends_with("shard.ir.pb")),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("is not a valid DocIr shard"),
        "{error}"
    );
}

fn anchor_site(name: &str, guide_links: &str) -> Result<(), CheckError> {
    let guard = dx_test_scratch::scratch(name).unwrap();
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(
        dir,
        "api.md",
        "# API Reference\n\n## rust_library\n\n- [`demo::T`](api/rust/demo/T.md)\n",
    );
    write_shard(dir, &["rust:demo:T"]);
    write(
        dir,
        "guide.md",
        &format!("# Guide\n\n## `--affected`\n\n## 中文\n\n{guide_links}\n"),
    );
    check(&inputs(dir))
}

#[test]
fn an_anchor_with_underscores_or_dashes_resolves() {
    let result = anchor_site("anchor-ok", "[a](api.md#rust_library) [b](#--affected)");
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_fragment_without_underscores_is_dangling() {
    let error = anchor_site("frag-bad", "[a](api.md#dxrustcrate)").unwrap_err();
    assert!(
        matches!(error, CheckError::DanglingFragment { .. }),
        "{error:?}"
    );
    assert_eq!(error.to_string(), "dangling fragment 'api.md#dxrustcrate'");
}

#[test]
fn a_collapsed_dash_anchor_is_dangling() {
    let error = anchor_site("anchor-bad", "[a](#-affected)").unwrap_err();
    assert!(
        matches!(error, CheckError::DanglingAnchor { .. }),
        "{error:?}"
    );
    assert_eq!(error.to_string(), "dangling anchor '#-affected'");
}

#[test]
fn a_fragment_of_the_wrong_case_is_dangling() {
    let error = anchor_site("frag-case", "[a](api.md#DX_RUST_CRATE)").unwrap_err();
    assert!(
        matches!(error, CheckError::DanglingFragment { .. }),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "dangling fragment 'api.md#DX_RUST_CRATE'"
    );
}

#[test]
fn a_percent_encoded_anchor_resolves() {
    let result = anchor_site("anchor-pct", "[a](#%E4%B8%AD%E6%96%87)");
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_site_with_nothing_linked_holds_together() {
    let guard = dx_test_scratch::scratch("plain").unwrap();
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(
        dir,
        "api.md",
        "# API Reference\n\n- [`demo::T`](api/rust/demo/T.md)\n",
    );
    write_shard(dir, &["rust:demo:T"]);
    write(dir, "guide.md", "# Guide\n\nPlain prose with no links.\n");
    let result = check(&inputs(dir));
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn each_unreadable_input_is_reported_with_its_path() {
    for missing in [
        "book.toml",
        "api.md",
        "guide.md",
        "shard.ir.pb",
        "records.json",
    ] {
        let guard = dx_test_scratch::scratch("unreadable").unwrap();
        let dir = guard.path();
        write(dir, "book.toml", "title = \"demo\"\n");
        write(
            dir,
            "api.md",
            "# API\n\n- [`demo::T`](api/rust/demo/T.md)\n",
        );
        write_shard(dir, &["rust:demo:T"]);
        write(dir, "guide.md", "# Guide\n");
        write(dir, "records.json", "[]");
        std::fs::remove_file(dir.join(missing)).expect("remove");
        let mut inputs = inputs(dir);
        inputs.records = Some(dir.join("records.json").to_string_lossy().into_owned());
        let error = check(&inputs).unwrap_err();
        let CheckError::Read(read) = &error else {
            panic!("{missing}: {error:?}");
        };
        assert_eq!(read.path(), dir.join(missing), "{error}");
        assert!(
            std::error::Error::source(&error).is_some(),
            "{missing}: {error:?}"
        );
    }
}

fn records_site(name: &str, records: &str) -> Result<(), CheckError> {
    let guard = dx_test_scratch::scratch(name).unwrap();
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(
        dir,
        "api.md",
        "# API\n\n- [`demo::T`](api/rust/demo/T.md)\n",
    );
    write_shard(dir, &["rust:demo:T"]);
    write(dir, "guide.md", "# Guide\n");
    write(dir, "records.json", records);
    let mut inputs = inputs(dir);
    inputs.records = Some(dir.join("records.json").to_string_lossy().into_owned());
    check(&inputs)
}

#[test]
fn a_search_record_url_must_resolve_to_a_rendered_page() {
    let good = "[{\"url\": \"api/rust/demo/T.html\", \"title\": \"demo::T\"}]";
    let result = records_site("records-ok", good);
    assert!(result.is_ok(), "{result:?}");
    let bad = "[{\"title\": \"x\", \"url\": \"api/rust/demo/Gone.html\"}]";
    let error = records_site("records-bad", bad).unwrap_err();
    assert!(
        matches!(&error, CheckError::RecordUrl { url } if url == "api/rust/demo/Gone.html"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "search record url 'api/rust/demo/Gone.html' has no rendered page"
    );
}

#[test]
fn links_inside_a_code_fence_are_not_checked() {
    let text = "[a](a.md)\n```powershell\n[Environment]::Set('x')\n[b](b.md)\n```\n[c](c.md)\n";
    let found = link_targets(&["guide.md".to_string()], &[text.to_string()], "");
    let targets: Vec<&str> = found.iter().map(|(_, target)| target.as_str()).collect();
    assert_eq!(targets, ["a.md", "c.md"]);
}
