//! Tests for the site checker helpers.

use std::path::Path;

use documentation_ir::proto::{DocIr, Symbol};
use documentation_ir::{encode_shard, SCHEMA_MAJOR};

use super::{
    anchor_name, check, first_field, has_empty_link_target, inline_targets, parse_args,
    resolve_route, split_reference, tidy, Inputs,
};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
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
        schema_major: SCHEMA_MAJOR,
        language: "rust".to_string(),
        package: "demo".to_string(),
        symbols: ids
            .iter()
            .map(|id| Symbol {
                id: id.to_string(),
                ..Symbol::default()
            })
            .collect(),
        ..DocIr::default()
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
    }
}

#[test]
fn an_anchor_matches_mdbook_ids() {
    assert_eq!(anchor_name("## Getting Started"), "getting-started");
    assert_eq!(anchor_name("# A/B and C"), "ab-and-c");
    assert_eq!(anchor_name("### Spaced -- Out"), "spaced----out");
    assert_eq!(anchor_name("#  Trim  Me  "), "trim--me");
    assert_eq!(anchor_name("## dx_rust_crate"), "dx_rust_crate");
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
    let guard = dx_test_scratch::scratch("empty-ids");
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(dir, "api.md", "# API\n");
    write_shard(dir, &[]);
    write(dir, "guide.md", "# Guide\n");
    assert_eq!(check(&inputs(dir)), "shard names no symbols");
}

#[test]
fn a_shard_that_does_not_decode_is_refused() {
    let guard = dx_test_scratch::scratch("bad-shard");
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(dir, "api.md", "# API\n");
    write(dir, "shard.ir.pb", "schema_major: 1\n");
    write(dir, "guide.md", "# Guide\n");
    let refusal = check(&inputs(dir));
    assert!(refusal.contains("is not a valid DocIr shard"), "{refusal}");
}

fn anchor_site(name: &str, guide_links: &str) -> String {
    let guard = dx_test_scratch::scratch(name);
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(
        dir,
        "api.md",
        "# API Reference\n\n## dx_rust_crate\n\n- [`demo::T`](api/rust/demo/T.md)\n",
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
    assert_eq!(
        anchor_site("anchor-ok", "[a](api.md#dx_rust_crate) [b](#--affected)"),
        ""
    );
}

#[test]
fn a_fragment_without_underscores_is_dangling() {
    assert_eq!(
        anchor_site("frag-bad", "[a](api.md#dxrustcrate)"),
        "dangling fragment 'api.md#dxrustcrate'"
    );
}

#[test]
fn a_collapsed_dash_anchor_is_dangling() {
    assert_eq!(
        anchor_site("anchor-bad", "[a](#-affected)"),
        "dangling anchor '#-affected'"
    );
}

#[test]
fn a_fragment_of_the_wrong_case_is_dangling() {
    assert_eq!(
        anchor_site("frag-case", "[a](api.md#DX_RUST_CRATE)"),
        "dangling fragment 'api.md#DX_RUST_CRATE'"
    );
}

#[test]
fn a_percent_encoded_anchor_resolves() {
    assert_eq!(anchor_site("anchor-pct", "[a](#%E4%B8%AD%E6%96%87)"), "");
}

#[test]
fn a_site_with_nothing_linked_holds_together() {
    let guard = dx_test_scratch::scratch("plain");
    let dir = guard.path();
    write(dir, "book.toml", "title = \"demo\"\n");
    write(
        dir,
        "api.md",
        "# API Reference\n\n- [`demo::T`](api/rust/demo/T.md)\n",
    );
    write_shard(dir, &["rust:demo:T"]);
    write(dir, "guide.md", "# Guide\n\nPlain prose with no links.\n");
    assert_eq!(check(&inputs(dir)), "");
}

#[test]
fn a_missing_book_file_is_reported_as_a_missing_title() {
    let inputs = Inputs {
        book: Some("absent.toml".into()),
        api: Some("absent.md".into()),
        shards: vec!["absent".into()],
        prose: vec!["absent.md".into()],
        data: vec![],
    };
    assert_eq!(check(&inputs), "book has no title");
}
