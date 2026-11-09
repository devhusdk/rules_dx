use super::*;
use serde_json::json;

fn obj(kind: &str, path: &str, parent: &str) -> Value {
    let name = path.rsplit('.').next().unwrap_or(path);
    json!({
        "kind": kind, "path": path, "name": name, "parent": parent,
        "doc": format!("Doc of {name}."), "signature": format!("{kind} {name}"),
        "file": "src/geo.f90", "line": 3, "params": [], "returns": null, "extends": ""
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value[key] = field;
    value
}

fn document(objects: Vec<Value>) -> String {
    json!({"format": 1, "ford": "7.0.13", "python": "3.12", "objects": objects}).to_string()
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

fn find<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing: {:?}", ids(shard)))
}

#[test]
fn converts_modules_types_and_procedures() {
    let function = with(
        with(
            obj("function", "geo.area", "geo"),
            "params",
            json!([{"name": "r", "type": "real", "intent": "in", "optional": false, "doc": "Radius."}]),
        ),
        "returns",
        json!({"type": "real", "doc": ""}),
    );
    let shard = convert(
        &document(vec![
            obj("module", "geo", ""),
            obj("type", "geo.shape_t", "geo"),
            obj("component", "geo.shape_t.name", "geo.shape_t"),
            function,
        ]),
        "geo",
    )
    .unwrap();
    assert_eq!(shard.language, "fortran");
    assert_eq!(
        ids(&shard),
        [
            "fortran:geo:geo",
            "fortran:geo:geo.area",
            "fortran:geo:geo.shape_t",
            "fortran:geo:geo.shape_t.name",
        ]
    );
    let area = find(&shard, "fortran:geo:geo.area");
    assert_eq!(area.kind, SymbolKind::Function as i32);
    assert_eq!(area.params[0].name, "r");
    assert_eq!(area.returns.as_ref().unwrap().r#type, "real");
    let name = find(&shard, "fortran:geo:geo.shape_t.name");
    assert_eq!(name.kind, SymbolKind::Field as i32);
    assert_eq!(
        name.relations.as_ref().unwrap().member_of,
        ["fortran:geo:geo.shape_t"]
    );
}

#[test]
fn marks_missing_docs_incomplete() {
    let bare = with(obj("subroutine", "geo.run", "geo"), "doc", json!(""));
    let shard = convert(&document(vec![obj("module", "geo", ""), bare]), "geo").unwrap();
    let run = find(&shard, "fortran:geo:geo.run");
    let note = run
        .extensions
        .iter()
        .find(|e| e.key == KEY_INCOMPLETE)
        .unwrap();
    assert_eq!(note.value, b"doc");
}

#[test]
fn links_extended_types() {
    let child = with(obj("type", "geo.sq_t", "geo"), "extends", json!("shape_t"));
    let shard = convert(
        &document(vec![
            obj("module", "geo", ""),
            obj("type", "geo.shape_t", "geo"),
            child,
        ]),
        "geo",
    )
    .unwrap();
    let child = find(&shard, "fortran:geo:geo.sq_t");
    assert_eq!(
        child.relations.as_ref().unwrap().implements,
        ["fortran:geo:geo.shape_t"]
    );
}

#[test]
fn constants_follow_the_parameter_attribute() {
    let constant = with(
        obj("variable", "geo.pi", "geo"),
        "signature",
        json!("real, parameter :: pi = 3.14"),
    );
    let shard = convert(
        &document(vec![obj("module", "geo", ""), constant, obj("variable", "geo.n", "geo")]),
        "geo",
    )
    .unwrap();
    assert_eq!(find(&shard, "fortran:geo:geo.pi").kind, SymbolKind::Constant as i32);
    assert_eq!(find(&shard, "fortran:geo:geo.n").kind, SymbolKind::Variable as i32);
}

#[test]
fn rejects_bad_input() {
    assert!(convert("{", "geo").unwrap_err().contains("malformed"));
    assert!(convert(&document(vec![]), "geo").unwrap_err().contains("no public"));
    assert!(convert(&document(vec![obj("module", "geo", "")]), "a b").is_err());
    let twice = document(vec![obj("module", "geo", ""), obj("module", "geo", "")]);
    assert!(convert(&twice, "geo").unwrap_err().contains("repeats"));
    let unknown = document(vec![obj("block", "geo", "")]);
    assert!(convert(&unknown, "geo").unwrap_err().contains("unknown kind"));
    let format = json!({"format": 2, "ford": "x", "objects": []}).to_string();
    assert!(convert(&format, "geo").unwrap_err().contains("not supported"));
}

#[test]
fn parses_arguments() {
    let args: Vec<String> = ["--json", "a", "--package", "p", "--out", "o"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(parse(&args).unwrap().package, "p");
    assert!(parse(&args[..4]).is_err());
    assert!(parse(&["--nope".to_owned()]).is_err());
}
