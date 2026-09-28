use super::*;
use crate::adapters::go::split_pos;
use crate::common::{shard, symbol_id};
use documentation_ir::proto::DocIr;

#[test]
fn adapters_validate_symbol_identity_and_unusual_input_shapes() {
    assert_eq!(shard("", "demo", vec![]), Err(AdapterError::EmptyIdentity));
    assert_eq!(
        symbol_id("rust", "demo", ""),
        Err(AdapterError::EmptyIdentity)
    );
    let mut ir = normalize_python(&fixture("python/input.json"), "demo").expect("fixture");
    ir.symbols.push(ir.symbols[0].clone());
    assert!(matches!(
        shard("python", "demo", ir.symbols),
        Err(AdapterError::DuplicateId(_))
    ));
    let mut python: serde_json::Value =
        serde_json::from_str(&fixture("python/input.json")).expect("fixture");
    python["members"][0]
        .as_object_mut()
        .expect("member")
        .remove("path");
    python["members"][0]
        .as_object_mut()
        .expect("member")
        .remove("name");
    assert!(matches!(
        normalize_python(&python.to_string(), "demo"),
        Err(AdapterError::InvalidJson(_))
    ));
    let mut typescript: serde_json::Value =
        serde_json::from_str(&fixture("typescript/input.json")).expect("fixture");
    for kind in [256, 1024, 32, 0] {
        typescript["children"][0]["kind"] = serde_json::json!(kind);
        assert!(!normalize_typescript(&typescript.to_string(), "demo")
            .expect("kind")
            .symbols
            .is_empty());
    }
    let mut scala: serde_json::Value =
        serde_json::from_str(&fixture("scala/input.json")).expect("fixture");
    scala["tasty"]["version"] = serde_json::json!("wrong");
    assert!(matches!(
        normalize_scala(&scala.to_string(), "demo"),
        Err(AdapterError::VersionMismatch { .. })
    ));
    assert_eq!(split_pos("source.go"), ("source.go".to_owned(), 1));
    let xml = format!("<doxygen version='{CPP_DOXYGEN_PIN}'><memberdef></memberdef><memberdef>");
    assert!(normalize_cpp(&xml, "demo")
        .expect("empty members")
        .symbols
        .is_empty());
}

#[test]
fn adapters_reject_empty_malformed_and_wrong_producer_inputs() {
    type Normalize = fn(&str, &str) -> Result<DocIr, AdapterError>;
    let adapters: [(&str, Normalize, &[&str]); 11] = [
        ("rust", normalize_rust, &["format_version"]),
        ("python", normalize_python, &["griffe_version"]),
        ("typescript", normalize_typescript, &["typedoc"]),
        ("java", normalize_java, &["jdk"]),
        ("kotlin", normalize_kotlin, &["dokka", "kotlin"]),
        ("go", normalize_go, &["go", "xtools"]),
        ("csharp", normalize_csharp, &["sdk"]),
        ("fsharp", normalize_fsharp, &["sdk", "fcs"]),
        ("vue", normalize_vue, &["docgen"]),
        ("svelte", normalize_svelte, &["sveld"]),
        ("scala", normalize_scala, &["scala"]),
    ];
    for (name, normalize, fields) in adapters {
        assert_eq!(
            normalize(" \n", "demo"),
            Err(AdapterError::EmptyInput),
            "{name}"
        );
        assert!(
            matches!(normalize("{", "demo"), Err(AdapterError::InvalidJson(_))),
            "{name}"
        );
        let original: serde_json::Value =
            serde_json::from_str(&fixture(&format!("{name}/input.json"))).expect("fixture");
        for field in fields {
            let mut input = original.clone();
            assert!(input.get(*field).is_some(), "{name} missing {field}");
            input[*field] = serde_json::json!("wrong-version");
            assert!(
                matches!(
                    normalize(&input.to_string(), "demo"),
                    Err(AdapterError::VersionMismatch { .. })
                ),
                "{name} {field}"
            );
        }
    }
    assert_eq!(normalize_cpp("", "demo"), Err(AdapterError::EmptyInput));
    assert!(matches!(
        normalize_cpp("<other/>", "demo"),
        Err(AdapterError::InvalidJson(_))
    ));
    assert!(matches!(
        normalize_cpp("<doxygen version=\"0\"/>", "demo"),
        Err(AdapterError::VersionMismatch { .. })
    ));
    assert_eq!(
        confirm_prose_only("", &[]),
        Err(AdapterError::EmptyIdentity)
    );
    assert!(matches!(
        confirm_prose_only("demo", &["/absolute.md"]),
        Err(AdapterError::AbsolutePath { .. })
    ));
}

#[test]
fn valid_json_with_object_shaped_vue_members_is_rejected() {
    for field in ["props", "events", "slots", "methods"] {
        let mut input: serde_json::Value =
            serde_json::from_str(&fixture("vue/input.json")).expect("fixture");
        input["components"][0][field] = serde_json::json!({});
        assert_eq!(
            normalize_vue(&input.to_string(), "web"),
            Err(AdapterError::InvalidJson(format!(
                "{field} must be an array"
            )))
        );
    }
}

fn fixture(name: &str) -> String {
    match name {
        "rust/input.json" => include_str!("../testdata/rust/input.json").to_owned(),
        "python/input.json" => include_str!("../testdata/python/input.json").to_owned(),
        "typescript/input.json" => include_str!("../testdata/typescript/input.json").to_owned(),
        "java/input.json" => include_str!("../testdata/java/input.json").to_owned(),
        "kotlin/input.json" => include_str!("../testdata/kotlin/input.json").to_owned(),
        "go/input.json" => include_str!("../testdata/go/input.json").to_owned(),
        "cpp/input.xml" => include_str!("../testdata/cpp/input.xml").to_owned(),
        "csharp/input.json" => include_str!("../testdata/csharp/input.json").to_owned(),
        "fsharp/input.json" => include_str!("../testdata/fsharp/input.json").to_owned(),
        "vue/input.json" => include_str!("../testdata/vue/input.json").to_owned(),
        "svelte/input.json" => include_str!("../testdata/svelte/input.json").to_owned(),
        "scala/input.json" => include_str!("../testdata/scala/input.json").to_owned(),
        _ => panic!("missing fixture {name}"),
    }
}

#[test]
fn rust_normalizes_pinned_index_with_overloads() {
    let shard = normalize_rust(&fixture("rust/input.json"), "demo").unwrap();
    assert_eq!(shard.language, "rust");
    assert_eq!(shard.symbols.len(), 4);
    assert!(shard.symbols.windows(2).all(|pair| pair[0].id < pair[1].id));
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn rust_rejects_unpinned_producer() {
    let bad = fixture("rust/input.json").replace(RUST_RUSTDOC_PIN, "nightly-2020-01-01");
    assert!(matches!(
        normalize_rust(&bad, "demo"),
        Err(AdapterError::VersionMismatch { .. })
    ));
}

#[test]
fn python_normalizes_griffe_members() {
    let shard = normalize_python(&fixture("python/input.json"), "mylib").unwrap();
    assert_eq!(shard.symbols.len(), 4);
    assert!(shard.symbols.windows(2).all(|pair| pair[0].id < pair[1].id));
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn typescript_normalizes_typedoc_reflections() {
    let shard = normalize_typescript(&fixture("typescript/input.json"), "web").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn java_normalizes_doclet_elements() {
    let shard = normalize_java(&fixture("java/input.json"), "example").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn kotlin_normalizes_dokka_declarations() {
    let shard = normalize_kotlin(&fixture("kotlin/input.json"), "example").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn go_normalizes_packages_with_positions() {
    let shard = normalize_go(&fixture("go/input.json"), "example").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn cpp_normalizes_doxygen_xml() {
    let shard = normalize_cpp(&fixture("cpp/input.xml"), "native").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn csharp_joins_metadata_with_docs() {
    let shard = normalize_csharp(&fixture("csharp/input.json"), "Example").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    assert!(shard
        .symbols
        .iter()
        .any(|symbol| symbol.doc_markdown.is_empty()));
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn fsharp_joins_service_with_docs() {
    let shard = normalize_fsharp(&fixture("fsharp/input.json"), "Example").unwrap();
    assert_eq!(shard.symbols.len(), 2);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn vue_requires_arrays_only() {
    let shard = normalize_vue(&fixture("vue/input.json"), "web").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bad = fixture("vue/input.json").replace("\"props\": [", "\"props\": {\"oops\": ");
    assert!(normalize_vue(&bad, "web").is_err());
}

#[test]
fn svelte_normalizes_components() {
    let shard = normalize_svelte(&fixture("svelte/input.json"), "web").unwrap();
    assert_eq!(shard.symbols.len(), 3);
    let bytes = encode_ir(&shard).unwrap();
    assert_eq!(encode_ir(&shard).unwrap(), bytes);
}

#[test]
fn scala_spike_fails_closed_without_tasty() {
    let shard = normalize_scala(&fixture("scala/input.json"), "example").unwrap();
    assert_eq!(shard.symbols.len(), 2);
    let missing = r#"{"scala": "3.3.6", "tasty": {"missing": true}}"#;
    assert_eq!(
        normalize_scala(missing, "example"),
        Err(AdapterError::MissingTasty)
    );
    let empty = r#"{"scala": "3.3.6", "tasty": {"version": "3.3.6", "symbols": []}}"#;
    assert_eq!(
        normalize_scala(empty, "example"),
        Err(AdapterError::MissingTasty)
    );
}

#[test]
fn astromdx_confirms_prose_only() {
    let shard =
        confirm_prose_only("site", &["docs/guide.md", "blog/post.mdx", "pages/a.astro"]).unwrap();
    assert!(shard.symbols.is_empty());
    assert_eq!(
        confirm_prose_only("site", &["src/main.rs"]),
        Err(AdapterError::NonMarkdown("src/main.rs".to_owned()))
    );
}

#[test]
fn adapters_reject_absolute_paths() {
    let bad = fixture("python/input.json").replace("src/account.py", "/abs/account.py");
    assert!(matches!(
        normalize_python(&bad, "mylib"),
        Err(AdapterError::AbsolutePath { .. })
    ));
}

#[test]
fn thirteen_scopes_stay_pinned() {
    assert_eq!(ADAPTER_SCOPES.len(), 13);
    assert!(ADAPTER_SCOPES.contains(&"scala"));
    assert!(ADAPTER_SCOPES.contains(&"astromdx"));
}
