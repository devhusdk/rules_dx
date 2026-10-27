use super::*;
use serde_json::json;

const P: &str = "ruby:geo:";

fn obj(kind: &str, path: &str, name: &str, namespace: &str) -> Value {
    json!({
        "path": path, "type": kind, "name": name, "namespace": namespace,
        "scope": if kind == "method" { "instance" } else { "" },
        "visibility": "public", "signature": format!("def {name}"),
        "docstring": format!("Doc of {name}."), "tags": [],
        "file": "lib/geo.rb", "generated": false, "line": 3, "owner": "",
        "superclass": "", "includes": [], "extends": [], "parameters": [],
        "attribute": null, "value": ""
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value[key] = field;
    value
}

fn tag(tag: &str, name: &str, types: &[&str], text: &str) -> Value {
    json!({"tag": tag, "name": name, "types": types, "text": text})
}

fn returning(value: Value) -> Value {
    with(
        value,
        "tags",
        json!([tag("return", "", &["Float"], "A number.")]),
    )
}

fn document(objects: Vec<Value>) -> Value {
    json!({"format": 1, "ruby": "3.4.9", "yard": "0.9.45", "objects": objects})
}

fn base() -> Vec<Value> {
    vec![
        obj("module", "Geo", "Geo", ""),
        obj("class", "Geo::Circle", "Circle", "Geo"),
    ]
}

fn convert_all(objects: Vec<Value>) -> DocIr {
    convert(&document(objects).to_string(), "geo").unwrap()
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let id = format!("{P}{qualified}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing: {:?}", ids(shard)))
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

fn extension<'a>(symbol: &'a Symbol, key: &str) -> Option<&'a str> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| std::str::from_utf8(&e.value).unwrap())
}

#[test]
fn segments_escape_every_byte_outside_the_word_set() {
    for (raw, escaped) in [
        ("area", "area"),
        ("empty?", "empty~3F"),
        ("save!", "save~21"),
        ("label=", "label~3D"),
        ("[]", "~5B~5D"),
        ("<=>", "~3C~3D~3E"),
        ("~", "~7E"),
        ("été", "~C3~A9t~C3~A9"),
        ("`", "~60"),
    ] {
        assert_eq!(escape_segment(raw), escaped, "{raw}");
    }
}

#[test]
fn every_kind_maps_with_its_display_name() {
    let mut objects = base();
    objects.push(with(
        obj("constant", "Geo::LIMIT", "LIMIT", "Geo"),
        "value",
        json!("10\n# trailing"),
    ));
    objects.push(returning(obj(
        "method",
        "Geo::Circle#empty?",
        "empty?",
        "Geo::Circle",
    )));
    objects.push(returning(with(
        obj("method", "Geo::Circle.unit", "unit", "Geo::Circle"),
        "scope",
        json!("class"),
    )));
    let shard = convert_all(objects);
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("ruby", "geo")
    );
    for (qualified, kind, display, signature) in [
        ("Geo", SymbolKind::Module, "Geo", "module Geo"),
        (
            "Geo.Circle",
            SymbolKind::Class,
            "Geo::Circle",
            "class Geo::Circle",
        ),
        (
            "Geo.LIMIT",
            SymbolKind::Constant,
            "Geo::LIMIT",
            "Geo::LIMIT = 10",
        ),
        (
            "Geo.Circle.empty~3F",
            SymbolKind::Method,
            "Geo::Circle#empty?",
            "def empty?",
        ),
        (
            "Geo.Circle.self.unit",
            SymbolKind::Method,
            "Geo::Circle.unit",
            "def self.unit",
        ),
    ] {
        let symbol = symbol(&shard, qualified);
        assert_eq!(symbol.kind(), kind, "{qualified}");
        assert_eq!(symbol.display_name, display, "{qualified}");
        assert_eq!(symbol.signature_text, signature, "{qualified}");
        assert_eq!(
            extension(symbol, KEY_CONFIG),
            Some("ruby=3.4.9;yard=0.9.45;format=1")
        );
    }
    let unit = symbol(&shard, "Geo.Circle.self.unit");
    assert_eq!(
        unit.relations.as_ref().unwrap().member_of,
        vec![format!("{P}Geo.Circle")]
    );
    let sorted: Vec<&str> = {
        let mut sorted = ids(&shard);
        sorted.sort();
        sorted
    };
    assert_eq!(ids(&shard), sorted);
}

#[test]
fn a_class_signature_names_its_superclass() {
    let mut objects = base();
    objects[1] = with(objects[1].clone(), "superclass", json!("Geo::Shape"));
    let shard = convert_all(objects);
    assert_eq!(
        symbol(&shard, "Geo.Circle").signature_text,
        "class Geo::Circle < Geo::Shape"
    );
}

fn attribute(name: &str, method: &str, read: bool, write: bool) -> Value {
    let path = format!("Geo::Circle#{method}");
    with(
        obj("method", &path, method, "Geo::Circle"),
        "attribute",
        json!({"name": name, "read": read, "write": write}),
    )
}

#[test]
fn attribute_pairs_become_one_property() {
    let mut objects = base();
    objects.push(returning(attribute("radius", "radius", true, false)));
    objects.push(returning(attribute("label", "label", true, true)));
    objects.push(attribute("label", "label=", true, true));
    objects.push(attribute("secret", "secret=", false, true));
    let shard = convert_all(objects);
    let radius = symbol(&shard, "Geo.Circle.radius");
    assert_eq!(radius.kind(), SymbolKind::Property);
    assert_eq!(radius.signature_text, "attr_reader :radius");
    assert_eq!(radius.display_name, "Geo::Circle#radius");
    assert_eq!(radius.returns.as_ref().unwrap().r#type, "Float");
    let label = symbol(&shard, "Geo.Circle.label");
    assert_eq!(label.signature_text, "attr_accessor :label");
    assert!(!ids(&shard).contains(&"ruby:geo:Geo.Circle.label~3D"));
    let secret = symbol(&shard, "Geo.Circle.secret~3D");
    assert_eq!(secret.signature_text, "attr_writer :secret");
    assert_eq!(secret.display_name, "Geo::Circle#secret=");
    assert!(secret.params.is_empty());
    assert_eq!(extension(secret, KEY_INCOMPLETE), None);
}

#[test]
fn only_the_public_api_of_the_own_files_is_emitted() {
    let mut objects = base();
    objects.push(with(
        obj("method", "Geo::Circle#hide", "hide", "Geo::Circle"),
        "visibility",
        json!("private"),
    ));
    objects.push(with(
        obj("method", "Geo::Circle#guard", "guard", "Geo::Circle"),
        "visibility",
        json!("protected"),
    ));
    objects.push(with(
        obj("method", "Geo::Circle#cache", "cache", "Geo::Circle"),
        "tags",
        json!([tag("api", "", &[], "private")]),
    ));
    objects.push(with(
        obj("method", "Geo::Circle#tagged", "tagged", "Geo::Circle"),
        "tags",
        json!([tag("private", "", &[], "")]),
    ));
    objects.push(with(
        obj("class", "Geo::Internal", "Internal", "Geo"),
        "tags",
        json!([tag("api", "", &[], "private")]),
    ));
    objects.push(obj("method", "Geo::Internal#run", "run", "Geo::Internal"));
    objects.push(with(
        obj("class", "Dep::Base", "Base", "Dep"),
        "owner",
        json!("dep"),
    ));
    let shard = convert_all(objects);
    assert_eq!(ids(&shard), vec!["ruby:geo:Geo", "ruby:geo:Geo.Circle"]);
}

#[test]
fn missing_facts_are_listed_as_incomplete() {
    let mut objects = base();
    objects.push(with(
        with(
            obj("method", "Geo::Circle#shift", "shift", "Geo::Circle"),
            "parameters",
            json!(["by", "*rest", "scale:", "**opts", "&block"]),
        ),
        "tags",
        json!([
            tag("param", "by", &["Float"], "Distance."),
            tag("param", "rest", &[], "More."),
            tag("return", "", &[], "Moved.")
        ]),
    ));
    objects.push(with(
        obj("method", "Geo::Circle#bare", "bare", "Geo::Circle"),
        "docstring",
        json!(""),
    ));
    objects.push(with(
        obj(
            "method",
            "Geo::Circle#initialize",
            "initialize",
            "Geo::Circle",
        ),
        "tags",
        json!([]),
    ));
    let shard = convert_all(objects);
    let shift = symbol(&shard, "Geo.Circle.shift");
    let names: Vec<(&str, &str, &str)> = shift
        .params
        .iter()
        .map(|p| (p.name.as_str(), p.r#type.as_str(), p.doc.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("by", "Float", "Distance."),
            ("rest", "", "More."),
            ("scale", "", ""),
            ("opts", "", "")
        ]
    );
    assert_eq!(
        extension(shift, KEY_INCOMPLETE),
        Some("params=scale,opts;param_types=rest;return_type")
    );
    assert_eq!(shift.returns.as_ref().unwrap().doc, "Moved.");
    let bare = symbol(&shard, "Geo.Circle.bare");
    assert_eq!(extension(bare, KEY_INCOMPLETE), Some("doc;return_type"));
    let init = symbol(&shard, "Geo.Circle.initialize");
    assert_eq!(extension(init, KEY_INCOMPLETE), None);
    assert_eq!(
        extension(symbol(&shard, "Geo.Circle"), KEY_INCOMPLETE),
        None
    );
}

#[test]
fn docs_normalize_crlf_line_endings() {
    let mut objects = base();
    objects[1] = with(
        objects[1].clone(),
        "docstring",
        json!("One.\r\n\r\nTwo.\r\n"),
    );
    let shard = convert_all(objects);
    assert_eq!(symbol(&shard, "Geo.Circle").doc_markdown, "One.\n\nTwo.");
}

#[test]
fn mixins_resolve_only_to_own_or_dependency_namespaces() {
    let mut objects = base();
    objects.push(obj("module", "Geo::Sized", "Sized", "Geo"));
    objects.push(with(
        obj("module", "Dep::Mixin", "Mixin", "Dep"),
        "owner",
        json!("dep"),
    ));
    objects[1] = with(
        with(
            objects[1].clone(),
            "includes",
            json!(["Geo::Sized", "Comparable"]),
        ),
        "extends",
        json!(["Dep::Mixin", "Unknown"]),
    );
    let shard = convert_all(objects);
    assert_eq!(
        symbol(&shard, "Geo.Circle")
            .relations
            .as_ref()
            .unwrap()
            .implements,
        vec!["ruby:dep:Dep.Mixin".to_owned(), format!("{P}Geo.Sized")]
    );
}

#[test]
fn a_member_of_a_dependency_namespace_points_at_the_dependency() {
    let objects = vec![
        with(obj("module", "Geo", "Geo", ""), "owner", json!("dep")),
        obj("class", "Geo::Square", "Square", "Geo"),
    ];
    let shard = convert_all(objects);
    assert_eq!(ids(&shard), vec!["ruby:geo:Geo.Square"]);
    assert_eq!(
        shard.symbols[0].relations.as_ref().unwrap().member_of,
        vec!["ruby:dep:Geo".to_owned()]
    );
}

#[test]
fn every_malformed_document_is_rejected() {
    let good = document(base());
    let cases = [
        (
            with(good.clone(), "format", json!(2)).to_string(),
            "format 2 is not supported",
        ),
        (good.to_string()[..40].to_owned(), "malformed"),
        ("[]".to_owned(), "not an object"),
        (
            document(vec![obj("classvariable", "Geo::@@x", "@@x", "Geo")]).to_string(),
            "unknown type 'classvariable'",
        ),
        (
            document(vec![with(
                obj("module", "Geo", "Geo", ""),
                "line",
                json!("3"),
            )])
            .to_string(),
            "'line' is not a number",
        ),
        (
            document(vec![with(
                obj("module", "Geo", "Geo", ""),
                "visibility",
                json!("private"),
            )])
            .to_string(),
            "no public Ruby API",
        ),
        (document(vec![]).to_string(), "no public Ruby API"),
    ];
    for (json, needle) in cases {
        let error = convert(&json, "geo").unwrap_err();
        assert!(error.contains(needle), "{needle}: {error}");
    }
    for package in ["", "a:b", "a b"] {
        assert!(convert(&good.to_string(), package).is_err(), "{package:?}");
    }
}

#[test]
fn an_id_collision_fails_validation() {
    let mut objects = base();
    objects.push(obj("constant", "Geo::Circle", "Circle", "Geo"));
    let error = convert(&document(objects).to_string(), "geo").unwrap_err();
    assert!(error.contains("DuplicateSymbolId"), "{error}");
}

#[test]
fn run_writes_no_shard_on_failure() {
    let dir = std::env::temp_dir().join(format!("ruby-extract-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let json = dir.join("in.json");
    let out = dir.join("out.ir.pb");
    fs::write(
        &json,
        with(document(base()), "format", json!(2)).to_string(),
    )
    .unwrap();
    let options = Options {
        json: json.display().to_string(),
        package: "geo".to_owned(),
        out: out.display().to_string(),
    };
    assert!(run(&options).is_err());
    assert!(!out.exists());
    fs::write(&json, document(base()).to_string()).unwrap();
    run(&options).unwrap();
    let shard = documentation_ir::decode_shard(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(shard.symbols.len(), 2);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn arguments_parse_strictly() {
    let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        parse(&args(&["--json", "a", "--package", "p", "--out", "b"])).unwrap(),
        Options {
            json: "a".to_owned(),
            package: "p".to_owned(),
            out: "b".to_owned(),
        }
    );
    for bad in [
        &["--json", "a", "--out", "b"][..],
        &["--json", "a", "--package", "p", "--out"][..],
        &["--json", "a", "--json", "b", "--package", "p", "--out", "c"][..],
        &["--bogus", "a"][..],
        &["--json", "", "--package", "p", "--out", "c"][..],
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?}");
    }
}
