//! Tests for the rustdoc JSON converter.

use serde_json::{json, Value};

use super::{extract, map_source, parse, parse_sources, ty, Options, Source};
use documentation_ir::proto::SymbolKind;
use rustdoc_types::{Type, FORMAT_VERSION};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn sources() -> Vec<Source> {
    vec![
        Source {
            exec_path: "pkg/src/lib.rs".to_string(),
            file: "pkg/src/lib.rs".to_string(),
            generated: false,
        },
        Source {
            exec_path: "bazel-out/k8/bin/pkg/build.out_dir/".to_string(),
            file: "pkg/build.out_dir/".to_string(),
            generated: true,
        },
    ]
}

fn span(file: &str, line: u64) -> Value {
    json!({"filename": file, "begin": [line, 1], "end": [line, 9]})
}

fn item(id: u32, name: Option<&str>, visibility: &str, docs: Option<&str>, inner: Value) -> Value {
    json!({
        "id": id,
        "crate_id": 0,
        "name": name,
        "span": span("pkg/src/lib.rs", u64::from(id) + 1),
        "visibility": visibility,
        "docs": docs,
        "links": {},
        "attrs": [],
        "deprecation": null,
        "inner": inner,
    })
}

fn no_generics() -> Value {
    json!({"params": [], "where_predicates": []})
}

fn function(inputs: Value, output: Value) -> Value {
    json!({"function": {
        "sig": {"inputs": inputs, "output": output, "is_c_variadic": false},
        "generics": no_generics(),
        "header": {"is_const": false, "is_unsafe": false, "is_async": false, "abi": "Rust"},
        "has_body": true,
    }})
}

fn crate_json(items: Vec<Value>, root_items: &[u32]) -> Value {
    let mut index = serde_json::Map::new();
    index.insert(
        "100".to_string(),
        item(
            100,
            Some("demo"),
            "public",
            Some("Crate docs."),
            json!({"module": {"is_crate": true, "items": root_items, "is_stripped": false}}),
        ),
    );
    for entry in items {
        index.insert(entry["id"].to_string(), entry);
    }
    json!({
        "root": 100,
        "crate_version": null,
        "includes_private": false,
        "index": index,
        "paths": {},
        "external_crates": {},
        "target": {"triple": "x86_64-unknown-linux-gnu", "target_features": []},
        "format_version": FORMAT_VERSION,
    })
}

fn demo() -> Value {
    let u64_ty = json!({"primitive": "u64"});
    crate_json(
        vec![
            item(
                1,
                Some("add"),
                "public",
                Some("Adds."),
                function(json!([["left", u64_ty], ["right", u64_ty]]), u64_ty.clone()),
            ),
            item(
                2,
                Some("hidden"),
                "default",
                None,
                function(json!([]), Value::Null),
            ),
            item(
                3,
                Some("shapes"),
                "public",
                None,
                json!({"module": {"is_crate": false, "items": [4], "is_stripped": false}}),
            ),
            item(
                4,
                Some("Circle"),
                "public",
                Some("A circle."),
                json!({"struct": {
                    "kind": {"plain": {"fields": [5], "has_stripped_fields": false}},
                    "generics": no_generics(),
                    "impls": [6],
                }}),
            ),
            item(
                5,
                Some("radius"),
                "public",
                None,
                json!({"struct_field": {"primitive": "f64"}}),
            ),
            item(
                6,
                None,
                "default",
                None,
                json!({"impl": {
                    "is_unsafe": false,
                    "generics": no_generics(),
                    "provided_trait_methods": [],
                    "trait": null,
                    "for": {"resolved_path": {"path": "Circle", "id": 4, "args": null}},
                    "items": [7],
                    "is_negative": false,
                    "is_synthetic": false,
                    "blanket_impl": null,
                }}),
            ),
            item(
                7,
                Some("area"),
                "public",
                Some("Area."),
                function(
                    json!([["self", {"borrowed_ref": {"lifetime": null, "is_mutable": false, "type": {"generic": "Self"}}}]]),
                    json!({"primitive": "f64"}),
                ),
            ),
            item(
                8,
                None,
                "public",
                None,
                json!({"use": {"source": "shapes::Circle", "name": "Circle", "id": 4, "is_glob": false}}),
            ),
        ],
        &[1, 2, 3, 8],
    )
}

fn run(value: &Value) -> Result<documentation_ir::proto::DocIr, String> {
    extract(&value.to_string(), "demo", "demo", &sources())
}

#[test]
fn parse_needs_every_flag_and_rejects_unknown_ones() {
    let full = args(&[
        "--json",
        "a.json",
        "--package",
        "p",
        "--crate",
        "c",
        "--srcs-file",
        "s",
        "--out",
        "o",
    ]);
    assert_eq!(
        parse(&full),
        Ok(Options {
            json: "a.json".to_string(),
            package: "p".to_string(),
            krate: "c".to_string(),
            srcs_file: "s".to_string(),
            out: "o".to_string(),
        })
    );
    assert_eq!(parse(&full[..8]), Err("--out is required".to_string()));
    assert_eq!(
        parse(&args(&["--json"])),
        Err("--json needs a value".to_string())
    );
    assert_eq!(
        parse(&args(&["--bogus", "x"])),
        Err("unknown argument '--bogus'".to_string())
    );
}

#[test]
fn sources_parse_and_map_by_exact_path_suffix_or_out_dir() {
    assert_eq!(parse_sources("a\tb\t1\nc\td\t0\n").map(|s| s.len()), Ok(2));
    assert_eq!(
        parse_sources("a\tb\n"),
        Err("srcs line needs three fields: a\tb".to_string())
    );
    assert_eq!(
        parse_sources("a\tb\tyes\n"),
        Err("srcs line has a bad generated flag 'yes'".to_string())
    );
    let sources = sources();
    assert_eq!(
        map_source(&sources, "pkg/src/lib.rs"),
        Some(("pkg/src/lib.rs".to_string(), false))
    );
    assert_eq!(
        map_source(&sources, "./pkg/src/lib.rs"),
        Some(("pkg/src/lib.rs".to_string(), false))
    );
    assert_eq!(
        map_source(&sources, "pkg/src\\lib.rs"),
        Some(("pkg/src/lib.rs".to_string(), false))
    );
    assert_eq!(
        map_source(&sources, "/execroot/_main/pkg/src/lib.rs"),
        Some(("pkg/src/lib.rs".to_string(), false))
    );
    assert_eq!(
        map_source(
            &sources,
            "/remote/bazel-out/k8/bin/pkg/build.out_dir/gen.rs"
        ),
        Some(("pkg/build.out_dir/gen.rs".to_string(), true))
    );
    assert_eq!(map_source(&sources, "pkg/src/other.rs"), None);
    assert_eq!(map_source(&sources, "xpkg/src/lib.rs"), None);
}

#[test]
fn public_items_become_sorted_symbols_with_signatures_and_aliases() {
    let shard = run(&demo()).expect("extract");
    assert_eq!(shard.language, "rust");
    assert_eq!(shard.package, "demo");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "rust:demo:add",
            "rust:demo:shapes",
            "rust:demo:shapes.Circle",
            "rust:demo:shapes.Circle.area",
            "rust:demo:shapes.Circle.radius",
        ]
    );
    let add = &shard.symbols[0];
    assert_eq!(
        add.signature_text,
        "pub fn add(left: u64, right: u64) -> u64"
    );
    assert_eq!(add.doc_markdown, "Adds.");
    assert_eq!(add.kind, SymbolKind::Function as i32);
    let source = add.source.as_ref().expect("source");
    assert_eq!(
        (source.file.as_str(), source.line, source.generated),
        ("pkg/src/lib.rs", 2, false)
    );
    let circle = &shard.symbols[2];
    assert_eq!(circle.signature_text, "pub struct Circle { .. }");
    let relations = circle.relations.as_ref().expect("relations");
    assert_eq!(
        relations.reexported_as,
        vec!["rust:demo:Circle".to_string()]
    );
    let area = &shard.symbols[3];
    assert_eq!(area.signature_text, "pub fn area(&self) -> f64");
    assert_eq!(area.kind, SymbolKind::Method as i32);
    assert_eq!(
        area.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec!["rust:demo:shapes.Circle".to_string()])
    );
    assert_eq!(shard.symbols[4].signature_text, "pub radius: f64");
}

#[test]
fn a_reexport_of_a_private_module_item_becomes_its_canonical_symbol() {
    let mut value = demo();
    value["index"]["3"]["visibility"] = json!("default");
    let shard = run(&value).expect("extract");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "rust:demo:Circle",
            "rust:demo:Circle.area",
            "rust:demo:Circle.radius",
            "rust:demo:add",
        ]
    );
    assert!(shard.symbols[0]
        .relations
        .as_ref()
        .is_none_or(|r| r.reexported_as.is_empty()));
}

#[test]
fn an_external_reexport_keeps_its_use_statement() {
    let mut value = demo();
    value["index"]["8"]["inner"]["use"] =
        json!({"source": "std::fmt::Write", "name": "Writer", "id": 999, "is_glob": false});
    let shard = run(&value).expect("extract");
    let writer = shard
        .symbols
        .iter()
        .find(|s| s.id == "rust:demo:Writer")
        .expect("re-export");
    assert_eq!(writer.signature_text, "pub use std::fmt::Write as Writer;");
    assert_eq!(writer.source.as_ref().map(|s| s.line), Some(9));
}

#[test]
fn bad_input_fails_without_a_shard() {
    let malformed = extract("{", "demo", "demo", &sources()).expect_err("malformed");
    assert!(
        malformed.starts_with("malformed rustdoc JSON"),
        "{malformed}"
    );

    let mut version = demo();
    version["format_version"] = json!(FORMAT_VERSION - 1);
    assert_eq!(
        run(&version),
        Err(format!(
            "rustdoc JSON format_version {} is not the supported {FORMAT_VERSION}",
            FORMAT_VERSION - 1
        ))
    );

    let mut missing = demo();
    missing["index"]["1"]["span"] = span("other/src/lib.rs", 4);
    assert_eq!(
        run(&missing),
        Err("source 'other/src/lib.rs' of 'add' is not a crate source".to_string())
    );

    let empty = crate_json(
        vec![item(
            2,
            Some("hidden"),
            "default",
            None,
            function(json!([]), Value::Null),
        )],
        &[2],
    );
    assert_eq!(
        run(&empty),
        Err("crate 'demo' has no public API".to_string())
    );

    assert_eq!(
        extract(&demo().to_string(), "demo", "other", &sources()),
        Err("rustdoc JSON documents 'demo', not crate 'other'".to_string())
    );
}

#[test]
fn types_print_as_source_text() {
    let parse_ty = |value: Value| -> Type { serde_json::from_value(value).expect("type") };
    assert_eq!(
        ty(&parse_ty(
            json!({"borrowed_ref": {"lifetime": "'a", "is_mutable": true, "type": {"slice": {"primitive": "u8"}}}})
        )),
        "&'a mut [u8]"
    );
    assert_eq!(
        ty(&parse_ty(
            json!({"resolved_path": {"path": "Vec", "id": 1, "args": {"angle_bracketed": {"args": [{"type": {"generic": "T"}}], "constraints": []}}}})
        )),
        "Vec<T>"
    );
    assert_eq!(
        ty(&parse_ty(json!({"tuple": [{"primitive": "u8"}]}))),
        "(u8,)"
    );
    assert_eq!(
        ty(&parse_ty(
            json!({"array": {"type": {"primitive": "u8"}, "len": "4"}})
        )),
        "[u8; 4]"
    );
    assert_eq!(
        ty(&parse_ty(
            json!({"raw_pointer": {"is_mutable": false, "type": {"generic": "T"}}})
        )),
        "*const T"
    );
}
