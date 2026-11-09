use super::*;
use serde_json::json;

fn object(kind: &str, path: &str, name: &str, namespace: &str, visibility: &str) -> Value {
    json!({
        "type": kind, "path": path, "name": name, "namespace": namespace,
        "signature": format!("sub {name}"), "docstring": format!("Doc of {name}."),
        "file": "lib/Geo.pm", "generated": false, "line": 3, "visibility": visibility
    })
}

fn document(objects: Vec<Value>) -> String {
    json!({"format": 1, "perl": "5.40.1", "objects": objects}).to_string()
}

#[test]
fn packages_and_subs_become_symbols() {
    let shard = convert(
        &document(vec![
            object("package", "Geo", "Geo", "", "public"),
            object("sub", "Geo::area", "area", "Geo", "public"),
        ]),
        "geo",
    )
    .unwrap();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["perl:geo:Geo", "perl:geo:Geo.area"]);
    assert_eq!(shard.symbols[0].kind, SymbolKind::Module as i32);
    assert_eq!(shard.symbols[1].kind, SymbolKind::Function as i32);
    let relations = shard.symbols[1].relations.as_ref().unwrap();
    assert_eq!(relations.member_of, ["perl:geo:Geo"]);
}

#[test]
fn private_subs_are_skipped() {
    let shard = convert(
        &document(vec![
            object("package", "Geo", "Geo", "", "public"),
            object("sub", "Geo::_hidden", "_hidden", "Geo", "private"),
        ]),
        "geo",
    )
    .unwrap();
    assert_eq!(shard.symbols.len(), 1);
}

#[test]
fn missing_docs_are_marked_incomplete() {
    let mut bare = object("package", "Geo", "Geo", "", "public");
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
            .contains("no public Perl API")
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
