use super::*;
use serde_json::json;

fn object(kind: &str, path: &str, name: &str, namespace: &str, visibility: &str) -> Value {
    json!({
        "type": kind, "path": path, "name": name, "namespace": namespace,
        "signature": format!("function {name}"), "docstring": format!("Doc of {name}."),
        "file": "src/Geo.php", "generated": false, "line": 3, "visibility": visibility
    })
}

fn document(objects: Vec<Value>) -> String {
    json!({"format": 1, "php": "8.5.0", "objects": objects}).to_string()
}

#[test]
fn classes_functions_and_methods_become_symbols() {
    let shard = convert(
        &document(vec![
            object("class", "Geo\\Shape", "Shape", "", "public"),
            object("method", "Geo\\Shape::area", "area", "Geo\\Shape", "public"),
            object("function", "Geo\\helper", "helper", "", "public"),
        ]),
        "geo",
    )
    .unwrap();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        ["php:geo:Geo.Shape", "php:geo:Geo.Shape.area", "php:geo:Geo.helper"]
    );
    assert_eq!(shard.symbols[0].kind, SymbolKind::Class as i32);
    assert_eq!(shard.symbols[1].kind, SymbolKind::Method as i32);
    assert_eq!(shard.symbols[2].kind, SymbolKind::Function as i32);
    let relations = shard.symbols[1].relations.as_ref().unwrap();
    assert_eq!(relations.member_of, ["php:geo:Geo.Shape"]);
}

#[test]
fn non_public_members_are_skipped() {
    let shard = convert(
        &document(vec![
            object("class", "Shape", "Shape", "", "public"),
            object("method", "Shape::hidden", "hidden", "Shape", "private"),
        ]),
        "geo",
    )
    .unwrap();
    assert_eq!(shard.symbols.len(), 1);
}

#[test]
fn missing_docs_are_marked_incomplete() {
    let mut bare = object("class", "Geo", "Geo", "", "public");
    bare["docstring"] = json!("");
    let shard = convert(&document(vec![bare]), "geo").unwrap();
    assert!(
        shard.symbols[0]
            .extensions
            .iter()
            .any(|e| e.key == KEY_INCOMPLETE && e.value == b"doc")
    );
}

#[test]
fn rejects_bad_input() {
    assert!(convert("{", "geo").unwrap_err().contains("malformed"));
    assert!(
        convert(&document(vec![]), "geo")
            .unwrap_err()
            .contains("no public PHP API")
    );
    assert!(
        convert(&document(vec![]), "a:b")
            .unwrap_err()
            .contains("invalid package")
    );
    let unknown = object("glob", "Geo", "Geo", "", "public");
    assert!(
        convert(&document(vec![unknown]), "geo")
            .unwrap_err()
            .contains("unknown type")
    );
}

#[test]
fn parse_requires_every_flag() {
    let args: Vec<String> = ["--json", "a", "--package", "p", "--out", "o"]
        .map(String::from)
        .to_vec();
    assert_eq!(parse(&args).unwrap().package, "p");
    assert!(parse(&args[..4]).unwrap_err().contains("--out is required"));
    assert!(parse(&["--x".to_owned()]).unwrap_err().contains("unknown"));
}
