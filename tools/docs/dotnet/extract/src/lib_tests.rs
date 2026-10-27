use super::*;
use documentation_ir::proto::SymbolKind;

fn settings() -> Settings {
    Settings {
        language: "csharp".to_owned(),
        package: "demo".to_owned(),
        config: String::new(),
        sources: vec![
            Source {
                exec_path: "lib/A.cs".to_owned(),
                file: "lib/A.cs".to_owned(),
                generated: false,
            },
            Source {
                exec_path: "bazel-out/gen/G.cs".to_owned(),
                file: "gen/G.cs".to_owned(),
                generated: true,
            },
        ],
    }
}

fn item(doc_id: &str, kind: &str, name: &str, parent: Option<&str>) -> String {
    let parent = parent.map_or("null".to_owned(), |p| format!("\"{p}\""));
    format!(
        r#"{{"doc_id":"{doc_id}","kind":"{kind}","name":"{name}","parent_doc_id":{parent},"summary":"Docs.","file":"lib/A.cs","line":3}}"#
    )
}

fn api(items: &[String]) -> String {
    format!(
        r#"{{"schema":"dx.dotnet-api/1","language":"csharp","assembly":"demo","items":[{}]}}"#,
        items.join(",")
    )
}

fn ns_item(name: &str) -> String {
    format!(r#"{{"doc_id":"N:{name}","kind":"namespace","name":"{name}","summary":"Ns."}}"#)
}

fn ids(json: &str) -> Vec<String> {
    convert(json, &settings())
        .unwrap()
        .symbols
        .into_iter()
        .map(|s| s.id)
        .collect()
}

#[test]
fn kinds_map_onto_symbol_kinds() {
    for (kind, expected) in [
        ("class", SymbolKind::Class),
        ("struct", SymbolKind::Class),
        ("record", SymbolKind::Class),
        ("interface", SymbolKind::Interface),
        ("enum", SymbolKind::Enum),
        ("union", SymbolKind::Enum),
        ("enum_field", SymbolKind::EnumVariant),
        ("union_case", SymbolKind::EnumVariant),
        ("method", SymbolKind::Method),
        ("constructor", SymbolKind::Method),
        ("operator", SymbolKind::Method),
        ("property", SymbolKind::Property),
        ("indexer", SymbolKind::Property),
        ("event", SymbolKind::Property),
        ("field", SymbolKind::Field),
        ("const", SymbolKind::Constant),
        ("delegate", SymbolKind::TypeAlias),
        ("abbreviation", SymbolKind::TypeAlias),
        ("namespace", SymbolKind::Module),
        ("module", SymbolKind::Module),
        ("function", SymbolKind::Function),
    ] {
        let json = api(&[item("T:A.X", kind, "X", None)]);
        let shard = convert(&json, &settings()).unwrap();
        assert_eq!(shard.symbols[0].kind, expected as i32, "{kind}");
    }
}

#[test]
fn unknown_kind_is_rejected() {
    let json = api(&[item("T:A.X", "weird", "X", None)]);
    assert!(convert(&json, &settings()).unwrap_err().contains("weird"));
}

#[test]
fn ids_escape_names_and_nest_under_parents() {
    let json = api(&[
        ns_item("Demo.Core"),
        item("T:Demo.Core.List`1", "class", "List`1", Some("N:Demo.Core")),
        item(
            "M:Demo.Core.List`1.#ctor",
            "constructor",
            "#ctor",
            Some("T:Demo.Core.List`1"),
        ),
    ]);
    assert_eq!(
        ids(&json),
        [
            "csharp:demo:Demo.Core",
            "csharp:demo:Demo.Core.List~601",
            "csharp:demo:Demo.Core.List~601.~23ctor",
        ]
    );
    let shard = convert(&json, &settings()).unwrap();
    let ctor = &shard.symbols[2];
    assert_eq!(
        ctor.relations.as_ref().unwrap().member_of,
        ["csharp:demo:Demo.Core.List~601"]
    );
}

fn overloads(order: &[&str]) -> Vec<String> {
    let mut items = vec![ns_item("A"), item("T:A.C", "class", "C", Some("N:A"))];
    for sig in order {
        items.push(item(
            &format!("M:A.C.Add({sig})"),
            "method",
            "Add",
            Some("T:A.C"),
        ));
    }
    items.push(item("M:A.C.Other", "method", "Other", Some("T:A.C")));
    ids(&api(&items))
}

#[test]
fn overload_suffix_is_stable_when_overloads_move_or_grow() {
    let two = overloads(&["System.Int32", "System.String"]);
    let swapped = overloads(&["System.String", "System.Int32"]);
    let three = overloads(&["System.String", "System.Int32", "System.Double"]);
    assert_eq!(two, swapped);
    for id in &two {
        assert!(three.contains(id), "{id}");
    }
    assert!(two.iter().any(|id| id == "csharp:demo:A.C.Other"));
    let suffixed: Vec<&String> = two.iter().filter(|id| id.contains("~~")).collect();
    assert_eq!(suffixed.len(), 2);
    for id in suffixed {
        let tail = id.rsplit("~~").next().unwrap();
        assert_eq!(tail.len(), 8);
        assert!(tail.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}

#[test]
fn case_folded_collisions_fail() {
    let json = api(&[
        item("T:A.Foo", "class", "Foo", None),
        item("T:A.foo", "class", "foo", None),
    ]);
    assert!(convert(&json, &settings())
        .unwrap_err()
        .contains("case folding"));
}

#[test]
fn duplicate_doc_ids_fail() {
    let json = api(&[
        item("T:A.Foo", "class", "Foo", None),
        item("T:A.Foo", "class", "Foo", None),
    ]);
    assert!(convert(&json, &settings())
        .unwrap_err()
        .contains("duplicate"));
}

#[test]
fn sources_map_through_the_manifest() {
    let json = format!(
        r#"{{"schema":"dx.dotnet-api/1","language":"csharp","assembly":"demo","items":[{{"doc_id":"T:A.G","kind":"class","name":"G","file":"bazel-out/gen/G.cs","line":7}},{}]}}"#,
        item("T:A.X", "class", "X", None)
    );
    let shard = convert(&json, &settings()).unwrap();
    let generated = shard.symbols.iter().find(|s| s.id.ends_with(":G")).unwrap();
    let source = generated.source.as_ref().unwrap();
    assert_eq!(
        (source.file.as_str(), source.line, source.generated),
        ("gen/G.cs", 7, true)
    );
    let plain = shard.symbols.iter().find(|s| s.id.ends_with(":X")).unwrap();
    assert!(!plain.source.as_ref().unwrap().generated);
}

#[test]
fn sources_outside_the_manifest_fail() {
    let json = api(&[item("T:A.X", "class", "X", None)]).replace("lib/A.cs", "lib/Other.cs");
    assert!(convert(&json, &settings())
        .unwrap_err()
        .contains("not a declared source"));
}

#[test]
fn incomplete_markers_and_missing_summaries_are_recorded() {
    let json = r#"{"schema":"dx.dotnet-api/1","language":"csharp","assembly":"demo","items":[{"doc_id":"T:A.X","kind":"class","name":"X","incomplete":["nullable annotations"]}]}"#;
    let shard = convert(json, &settings()).unwrap();
    let ext = shard.symbols[0]
        .extensions
        .iter()
        .find(|e| e.key == "csharp.incomplete")
        .unwrap();
    assert_eq!(ext.value, b"nullable annotations;summary");
}

#[test]
fn empty_api_is_rejected() {
    assert!(convert(&api(&[]), &settings())
        .unwrap_err()
        .contains("no public items"));
}

#[test]
fn malformed_and_foreign_inputs_are_rejected() {
    assert!(convert("{", &settings()).unwrap_err().contains("malformed"));
    let wrong = api(&[item("T:A.X", "class", "X", None)]).replace("api/1", "api/2");
    assert!(convert(&wrong, &settings())
        .unwrap_err()
        .contains("not supported"));
    let extra = api(&[item("T:A.X", "class", "X", None)])
        .replace("\"assembly\"", "\"extra\":1,\"assembly\"");
    assert!(convert(&extra, &settings())
        .unwrap_err()
        .contains("does not match"));
    let other = Settings {
        language: "fsharp".to_owned(),
        ..settings()
    };
    assert!(convert(&api(&[item("T:A.X", "class", "X", None)]), &other)
        .unwrap_err()
        .contains("language"));
}

#[test]
fn parse_requires_flags_and_known_language() {
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let full = [
        "--api",
        "a",
        "--language",
        "csharp",
        "--package",
        "p",
        "--srcs-file",
        "s",
        "--out",
        "o",
    ];
    let options = parse(&args(&full)).unwrap();
    assert_eq!(options.language, "csharp");
    assert!(parse(&args(&full[..8])).unwrap_err().contains("--out"));
    let mut bad = full.to_vec();
    bad[3] = "vb";
    assert!(parse(&args(&bad)).unwrap_err().contains("csharp or fsharp"));
    assert!(parse(&args(&["--nope", "x"]))
        .unwrap_err()
        .contains("unknown"));
}

#[test]
fn run_writes_nothing_on_error() {
    let dir = std::env::temp_dir().join(format!("dotnet_extract_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let api_path = dir.join("api.json");
    let srcs = dir.join("srcs.tsv");
    let out = dir.join("out.bin");
    std::fs::write(&api_path, "{").unwrap();
    std::fs::write(&srcs, "lib/A.cs\tlib/A.cs\t0\n").unwrap();
    let options = Options {
        api: api_path.display().to_string(),
        language: "csharp".to_owned(),
        package: "demo".to_owned(),
        srcs_file: srcs.display().to_string(),
        config: String::new(),
        out: out.display().to_string(),
    };
    assert!(run(&options).is_err());
    assert!(!out.exists());
    std::fs::write(&api_path, api(&[item("T:A.X", "class", "X", None)])).unwrap();
    run(&options).unwrap();
    assert!(documentation_ir::decode_shard(&std::fs::read(&out).unwrap()).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}
