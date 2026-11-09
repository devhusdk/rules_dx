use super::*;
use serde_json::json;

fn obj(kind: &str, path: &str, name: &str, module: &str) -> Value {
    json!({
        "path": path, "name": name, "module": module, "kind": kind,
        "signatures": [], "doc": format!("Doc of {name}."),
        "file": "src/Geo.jl", "generated": false, "line": 3
    })
}

fn document(objects: Vec<Value>) -> String {
    json!({"format": 1, "julia": "1.12.5", "documenter": "1.20.0", "objects": objects}).to_string()
}

fn base() -> Vec<Value> {
    vec![
        obj("module", "Geo", "Geo", ""),
        obj("struct", "Geo.Circle", "Circle", "Geo"),
        obj("function", "Geo.area", "area", "Geo"),
    ]
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == format!("julia:geo:{id}"))
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn converts_modules_types_and_functions() {
    let shard = convert(&document(base()), "geo").unwrap();
    assert_eq!(shard.language, "julia");
    assert_eq!(shard.symbols.len(), 3);
    assert_eq!(symbol(&shard, "Geo").kind, SymbolKind::Module as i32);
    assert_eq!(symbol(&shard, "Geo.Circle").kind, SymbolKind::Class as i32);
    let area = symbol(&shard, "Geo.area");
    assert_eq!(area.kind, SymbolKind::Function as i32);
    assert_eq!(area.doc_markdown, "Doc of area.");
    assert_eq!(
        area.relations.as_ref().unwrap().member_of,
        vec!["julia:geo:Geo".to_owned()]
    );
}

#[test]
fn signatures_are_joined_and_modules_get_a_header() {
    let mut objects = base();
    objects[2]["signatures"] = json!(["area(c::Circle)", "area(r::Rect)"]);
    let shard = convert(&document(objects), "geo").unwrap();
    assert_eq!(
        symbol(&shard, "Geo.area").signature_text,
        "area(c::Circle)\narea(r::Rect)"
    );
    assert_eq!(symbol(&shard, "Geo").signature_text, "module Geo");
}

#[test]
fn undocumented_symbols_are_marked_incomplete() {
    let mut objects = base();
    objects[1]["doc"] = json!("");
    let shard = convert(&document(objects), "geo").unwrap();
    let circle = symbol(&shard, "Geo.Circle");
    assert!(
        circle
            .extensions
            .iter()
            .any(|e| e.key == KEY_INCOMPLETE && e.value == b"doc")
    );
}

#[test]
fn rejects_bad_input() {
    assert!(convert("{", "geo").unwrap_err().contains("malformed"));
    let wrong = json!({"format": 2, "julia": "1", "documenter": "1", "objects": []}).to_string();
    assert!(convert(&wrong, "geo").unwrap_err().contains("format 2"));
    assert!(convert(&document(vec![]), "geo").unwrap_err().contains("no public"));
    assert!(convert(&document(base()), "a b").unwrap_err().contains("invalid package"));
    let mut objects = base();
    objects[0]["kind"] = json!("weird");
    assert!(convert(&document(objects), "geo").unwrap_err().contains("unknown kind"));
}

#[test]
fn parse_requires_every_flag_once() {
    let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let ok = parse(&args(&["--json", "a", "--package", "p", "--out", "o"])).unwrap();
    assert_eq!(ok.package, "p");
    assert!(parse(&args(&["--json", "a"])).unwrap_err().contains("required"));
    assert!(parse(&args(&["--bogus", "a"])).unwrap_err().contains("unknown"));
    assert!(parse(&args(&["--json"])).unwrap_err().contains("needs a value"));
    assert!(
        parse(&args(&["--json", "a", "--json", "b"]))
            .unwrap_err()
            .contains("more than once")
    );
    assert!(parse(&args(&["--json", ""])).unwrap_err().contains("empty"));
}
