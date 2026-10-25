use documentation_ir::{decode_shard, proto, SCHEMA_MAJOR};
use proto::{DocIr, Symbol, SymbolKind};

const SHARD: &str = "tools/docs/rust/fixture/docs_fixture_docs.ir.pb";
const CHANGED: &str = "tools/docs/rust/fixture/docs_fixture_changed_docs.ir.pb";

fn decoded(rel: &str) -> DocIr {
    let path = dx_testing::resolve_runfiles(rel);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {rel}: {error}"));
    decode_shard(&bytes).unwrap_or_else(|error| panic!("{rel} does not decode: {error:?}"))
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|symbol| symbol.id == id)
        .unwrap_or_else(|| panic!("no symbol {id}"))
}

#[test]
fn the_fixture_shard_names_every_public_item_once() {
    let shard = decoded(SHARD);
    assert_eq!(shard.schema_major, SCHEMA_MAJOR);
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("rust", "docs_fixture")
    );
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "rust:docs_fixture:GENERATED_VERSION",
            "rust:docs_fixture:add",
            "rust:docs_fixture:shapes",
            "rust:docs_fixture:shapes.Circle",
            "rust:docs_fixture:shapes.Circle.area",
            "rust:docs_fixture:shapes.Circle.radius",
            "rust:docs_fixture:shapes.unit_circle",
        ]
    );
}

#[test]
fn symbols_carry_signatures_docs_and_repo_sources() {
    let shard = decoded(SHARD);
    let add = symbol(&shard, "rust:docs_fixture:add");
    assert_eq!(add.kind, SymbolKind::Function as i32);
    assert_eq!(
        add.signature_text,
        "pub fn add(left: u64, right: u64) -> u64"
    );
    assert_eq!(add.doc_markdown, "Adds two lengths in meters.");
    let source = add.source.as_ref().expect("add has a source");
    assert_eq!(
        (source.file.as_str(), source.line, source.generated),
        ("tools/docs/rust/fixture/src/lib.rs", 8, false)
    );
    let area = symbol(&shard, "rust:docs_fixture:shapes.Circle.area");
    assert_eq!(area.kind, SymbolKind::Method as i32);
    assert_eq!(area.signature_text, "pub fn area(&self) -> f64");
    assert_eq!(
        area.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec!["rust:docs_fixture:shapes.Circle".to_string()])
    );
}

#[test]
fn a_crate_root_reexport_is_an_alias_of_the_canonical_symbol() {
    let shard = decoded(SHARD);
    let circle = symbol(&shard, "rust:docs_fixture:shapes.Circle");
    assert_eq!(
        circle.relations.as_ref().map(|r| r.reexported_as.clone()),
        Some(vec!["rust:docs_fixture:Circle".to_string()])
    );
}

#[test]
fn build_script_output_is_marked_generated() {
    let shard = decoded(SHARD);
    let generated = symbol(&shard, "rust:docs_fixture:GENERATED_VERSION");
    let source = generated
        .source
        .as_ref()
        .expect("generated item has a source");
    assert_eq!(
        source.file,
        "tools/docs/rust/fixture/docs_fixture_build_script.out_dir/generated.rs"
    );
    assert!(source.generated);
}

#[test]
fn the_crate_features_select_the_documented_items() {
    let shard = decoded(SHARD);
    assert!(shard
        .symbols
        .iter()
        .any(|s| s.id.ends_with("shapes.unit_circle")));
    assert!(!shard
        .symbols
        .iter()
        .any(|s| s.id.ends_with("without_extra")));
}

#[test]
fn a_changed_signature_reaches_the_shard() {
    let changed = decoded(CHANGED);
    let add = symbol(&changed, "rust:docs_fixture:add");
    assert_eq!(
        add.signature_text,
        "pub fn add(left: u64, right: u64, carry: u32) -> u64"
    );
    assert_eq!(
        add.doc_markdown,
        "Adds two lengths and a carry, wrapping on overflow."
    );
}
