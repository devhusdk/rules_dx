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

fn module_item(id: u32, name: &str, items: &[u32]) -> Value {
    item(
        id,
        Some(name),
        "public",
        None,
        json!({"module": {"is_crate": false, "items": items, "is_stripped": false}}),
    )
}

fn fn_item(id: u32, name: &str) -> Value {
    item(
        id,
        Some(name),
        "public",
        None,
        function(json!([]), Value::Null),
    )
}

fn struct_item(id: u32, name: &str, fields: &[u32], impls: &[u32]) -> Value {
    item(
        id,
        Some(name),
        "public",
        None,
        json!({"struct": {
            "kind": {"plain": {"fields": fields, "has_stripped_fields": false}},
            "generics": no_generics(),
            "impls": impls,
        }}),
    )
}

fn field_item(id: u32, name: &str) -> Value {
    item(
        id,
        Some(name),
        "public",
        None,
        json!({"struct_field": {"primitive": "f64"}}),
    )
}

fn impl_item(id: u32, owner: u32, items: &[u32]) -> Value {
    item(
        id,
        None,
        "default",
        None,
        json!({"impl": {
            "is_unsafe": false,
            "generics": no_generics(),
            "provided_trait_methods": [],
            "trait": null,
            "for": {"resolved_path": {"path": "T", "id": owner, "args": null}},
            "items": items,
            "is_negative": false,
            "is_synthetic": false,
            "blanket_impl": null,
        }}),
    )
}

fn use_item(id: u32, source: &str, name: &str, target: Option<u32>, glob: bool) -> Value {
    item(
        id,
        None,
        "public",
        None,
        json!({"use": {"source": source, "name": name, "id": target, "is_glob": glob}}),
    )
}

fn ids(value: &Value) -> Vec<String> {
    run(value)
        .expect("extract")
        .symbols
        .into_iter()
        .map(|symbol| symbol.id)
        .collect()
}

fn symbol(shard: &documentation_ir::proto::DocIr, id: &str) -> documentation_ir::proto::Symbol {
    shard
        .symbols
        .iter()
        .find(|symbol| symbol.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .clone()
}

#[test]
fn a_module_and_a_function_with_one_name_keep_the_type_id_plain() {
    let value = crate_json(vec![fn_item(1, "foo"), module_item(2, "foo", &[])], &[1, 2]);
    assert_eq!(ids(&value), vec!["rust:demo:foo", "rust:demo:foo@fn"]);
    let shard = run(&value).expect("extract");
    assert_eq!(
        symbol(&shard, "rust:demo:foo").kind,
        SymbolKind::Module as i32
    );
}

#[test]
fn a_field_and_methods_of_one_name_get_tags_in_source_order() {
    let value = crate_json(
        vec![
            struct_item(1, "Point", &[2], &[3, 5, 7]),
            field_item(2, "x"),
            impl_item(3, 1, &[4]),
            fn_item(4, "x"),
            impl_item(5, 1, &[6]),
            fn_item(6, "x"),
            impl_item(7, 1, &[8]),
            fn_item(8, "x"),
        ],
        &[1],
    );
    let expected = vec![
        "rust:demo:Point",
        "rust:demo:Point.x",
        "rust:demo:Point.x@method",
        "rust:demo:Point.x@method-2",
        "rust:demo:Point.x@method-3",
    ];
    assert_eq!(ids(&value), expected);
    let shard = run(&value).expect("extract");
    for id in &expected[1..] {
        let relations = symbol(&shard, id).relations.expect("relations");
        assert_eq!(relations.member_of, vec!["rust:demo:Point".to_string()]);
    }
    assert_eq!(
        symbol(&shard, "rust:demo:Point.x").kind,
        SymbolKind::Field as i32
    );
}

#[test]
fn a_variant_wins_over_a_method_of_the_same_name() {
    let value = crate_json(
        vec![
            item(
                1,
                Some("Kind"),
                "public",
                None,
                json!({"enum": {
                    "generics": no_generics(),
                    "has_stripped_variants": false,
                    "variants": [2],
                    "impls": [3],
                }}),
            ),
            item(
                2,
                Some("A"),
                "default",
                None,
                json!({"variant": {"kind": "plain", "discriminant": null}}),
            ),
            impl_item(3, 1, &[4]),
            fn_item(4, "A"),
        ],
        &[1],
    );
    assert_eq!(
        ids(&value),
        vec![
            "rust:demo:Kind",
            "rust:demo:Kind.A",
            "rust:demo:Kind.A@method"
        ]
    );
}

#[test]
fn ids_do_not_depend_on_the_order_rustdoc_lists_items() {
    let items = vec![
        struct_item(1, "Point", &[2], &[3, 5]),
        field_item(2, "x"),
        impl_item(3, 1, &[4]),
        fn_item(4, "x"),
        impl_item(5, 1, &[6]),
        fn_item(6, "x"),
        fn_item(9, "foo"),
        module_item(10, "foo", &[]),
    ];
    let forward = crate_json(items.clone(), &[1, 9, 10]);
    let mut shuffled = items;
    shuffled.reverse();
    let mut reversed = crate_json(shuffled, &[10, 9, 1]);
    reversed["index"]["1"]["inner"]["struct"]["impls"] = json!([5, 3]);
    assert_eq!(run(&forward).expect("a"), run(&reversed).expect("b"));
}

#[test]
fn a_reexport_in_another_namespace_gets_a_tag_and_is_listed() {
    let value = crate_json(
        vec![
            module_item(3, "foo", &[4]),
            fn_item(4, "foo"),
            use_item(8, "foo::foo", "foo", Some(4), false),
        ],
        &[3, 8],
    );
    assert_eq!(ids(&value), vec!["rust:demo:foo", "rust:demo:foo.foo"]);
    let shard = run(&value).expect("extract");
    let relations = symbol(&shard, "rust:demo:foo.foo")
        .relations
        .expect("relations");
    assert_eq!(
        relations.reexported_as,
        vec!["rust:demo:foo@fn".to_string()]
    );
}

#[test]
fn a_glob_of_a_local_enum_reexports_its_variants() {
    let value = crate_json(
        vec![
            item(
                1,
                Some("Kind"),
                "public",
                None,
                json!({"enum": {
                    "generics": no_generics(),
                    "has_stripped_variants": false,
                    "variants": [2, 3],
                    "impls": [],
                }}),
            ),
            item(
                2,
                Some("A"),
                "default",
                None,
                json!({"variant": {"kind": "plain", "discriminant": null}}),
            ),
            item(
                3,
                Some("B"),
                "default",
                None,
                json!({"variant": {"kind": "plain", "discriminant": null}}),
            ),
            use_item(8, "self::Kind", "Kind", Some(1), true),
        ],
        &[1, 8],
    );
    let shard = run(&value).expect("extract");
    assert_eq!(
        symbol(&shard, "rust:demo:Kind.A")
            .relations
            .expect("relations")
            .reexported_as,
        vec!["rust:demo:A".to_string()]
    );
    assert_eq!(
        symbol(&shard, "rust:demo:Kind.B")
            .relations
            .expect("relations")
            .reexported_as,
        vec!["rust:demo:B".to_string()]
    );
}

#[test]
fn a_glob_of_a_foreign_module_becomes_one_glob_symbol() {
    let value = crate_json(vec![use_item(8, "core::cmp", "cmp", None, true)], &[8]);
    let shard = run(&value).expect("extract");
    assert_eq!(
        shard
            .symbols
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["rust:demo:cmp@glob"]
    );
    assert_eq!(shard.symbols[0].signature_text, "pub use core::cmp::*;");
}

#[test]
fn glob_names_follow_rusts_shadowing_rules() {
    // An explicit local item shadows a glob name.
    let shadowed = crate_json(
        vec![
            module_item(1, "m", &[2]),
            struct_item(2, "S", &[], &[]),
            struct_item(3, "S", &[], &[]),
            use_item(8, "m", "m", Some(1), true),
        ],
        &[1, 3, 8],
    );
    let shard = run(&shadowed).expect("shadowed");
    assert!(symbol(&shard, "rust:demo:m.S")
        .relations
        .is_none_or(|r| r.reexported_as.is_empty()));
    // Two globs that give one name make that name unusable.
    let ambiguous = crate_json(
        vec![
            module_item(1, "a", &[2]),
            struct_item(2, "S", &[], &[]),
            module_item(3, "b", &[4]),
            struct_item(4, "S", &[], &[]),
            use_item(8, "a", "a", Some(1), true),
            use_item(9, "b", "b", Some(3), true),
        ],
        &[1, 3, 8, 9],
    );
    let shard = run(&ambiguous).expect("ambiguous");
    for id in ["rust:demo:a.S", "rust:demo:b.S"] {
        assert!(symbol(&shard, id)
            .relations
            .is_none_or(|r| r.reexported_as.is_empty()));
    }
}

#[test]
fn an_explicit_reexport_in_the_same_namespace_is_an_error() {
    let value = crate_json(
        vec![
            struct_item(1, "S", &[], &[]),
            module_item(2, "m", &[3]),
            struct_item(3, "T", &[], &[]),
            use_item(8, "m::T", "S", Some(3), false),
        ],
        &[1, 2, 8],
    );
    let error = run(&value).expect_err("collision");
    assert!(error.contains("rust:demo:S"), "{error}");
}
