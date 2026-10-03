//! Tests for the site checker helpers.

use std::path::{Path, PathBuf};

use dx_site_check::{
    anchor_name, check, first_field, has_empty_link_target, inline_targets, normalize_link,
    parse_args, split_reference, tidy, Inputs,
};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dx-site-check-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(path, body).expect("write");
}

#[test]
fn an_anchor_drops_punctuation_and_joins_words() {
    assert_eq!(anchor_name("## Getting Started"), "getting-started");
    assert_eq!(anchor_name("# A/B and C"), "ab-and-c");
    assert_eq!(anchor_name("### Spaced -- Out"), "spaced-out");
    assert_eq!(anchor_name("#  Trim  Me  "), "trim-me");
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
fn leading_parent_and_current_segments_come_off() {
    assert_eq!(normalize_link("../LICENSE"), "LICENSE");
    assert_eq!(normalize_link("../../a/b.md"), "a/b.md");
    assert_eq!(normalize_link("./api.md"), "api.md");
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
fn a_shard_with_only_empty_ids_names_no_symbols() {
    let dir = scratch("empty-ids");
    write(&dir, "book.toml", "title = \"demo\"\n");
    write(&dir, "api.md", "# API\n");
    write(&dir, "shard.ir.textproto", "symbols {\n  id: \"\"\n}\n");
    write(&dir, "guide.md", "# Guide\n");
    let inputs = Inputs {
        book: Some("book.toml".into()),
        api: Some("api.md".into()),
        shards: vec!["shard.ir.textproto".into()],
        prose: vec!["guide.md".into()],
        data: vec![],
    };
    assert_eq!(check(&inputs), "shard names no symbols");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_site_with_nothing_linked_holds_together() {
    let dir = scratch("plain");
    write(&dir, "book.toml", "title = \"demo\"\n");
    write(&dir, "api.md", "# API Reference\n\n## python:demo:T\n");
    write(&dir, "shard.ir.textproto", "symbols {\n  id: \"python:demo:T\"\n}\n");
    write(&dir, "guide.md", "# Guide\n\nPlain prose with no links.\n");
    let inputs = Inputs {
        book: Some("book.toml".into()),
        api: Some("api.md".into()),
        shards: vec!["shard.ir.textproto".into()],
        prose: vec!["guide.md".into()],
        data: vec![],
    };
    assert_eq!(check(&inputs), "");
    let _ = std::fs::remove_dir_all(&dir);
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