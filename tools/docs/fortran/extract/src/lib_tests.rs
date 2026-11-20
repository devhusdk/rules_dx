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
    json!({"format": 1, "ford": "7.0.13", "python": "3.12", "objects": Value::Array(objects)})
        .to_string()
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
        &document(vec![
            obj("module", "geo", ""),
            constant,
            obj("variable", "geo.n", "geo"),
        ]),
        "geo",
    )
    .unwrap();
    assert_eq!(
        find(&shard, "fortran:geo:geo.pi").kind,
        SymbolKind::Constant as i32
    );
    assert_eq!(
        find(&shard, "fortran:geo:geo.n").kind,
        SymbolKind::Variable as i32
    );
}

#[test]
fn rejects_bad_input() {
    let error = convert("{", "geo").unwrap_err();
    assert!(matches!(error, FortranExtractError::Malformed(_)));
    assert_eq!(
        error.to_string(),
        "FORD JSON is malformed: EOF while parsing an object at line 1 column 1"
    );
    let error = convert(&document(vec![]), "geo").unwrap_err();
    assert!(matches!(error, FortranExtractError::NoPublicApi { .. }));
    assert_eq!(
        error.to_string(),
        "FORD JSON lists no public Fortran API of package 'geo'"
    );
    let error = convert(&document(vec![obj("module", "geo", "")]), "a b").unwrap_err();
    assert!(matches!(error, FortranExtractError::InvalidPackage { .. }));
    assert_eq!(error.to_string(), "invalid package 'a b'");
    let twice = document(vec![obj("module", "geo", ""), obj("module", "geo", "")]);
    let error = convert(&twice, "geo").unwrap_err();
    assert!(matches!(error, FortranExtractError::RepeatedPath { .. }));
    assert_eq!(
        error.to_string(),
        "FORD JSON object 1 repeats the path 'geo'"
    );
    let unknown = document(vec![obj("block", "geo", "")]);
    let error = convert(&unknown, "geo").unwrap_err();
    assert!(matches!(error, FortranExtractError::UnknownKind { .. }));
    assert_eq!(
        error.to_string(),
        "FORD JSON object 0 has unknown kind 'block'"
    );
    let format = json!({"format": 2, "ford": "x", "objects": []}).to_string();
    let error = convert(&format, "geo").unwrap_err();
    assert!(matches!(
        error,
        FortranExtractError::UnsupportedFormat { found: 2 }
    ));
    assert_eq!(
        error.to_string(),
        "FORD JSON format 2 is not supported, expected 1"
    );
    let mut returns = obj("function", "geo.area", "geo");
    returns["returns"] = json!(1);
    let error = convert(&document(vec![returns]), "geo").unwrap_err();
    assert!(matches!(
        error,
        FortranExtractError::ReturnsNotObject { .. }
    ));
    assert_eq!(
        error.to_string(),
        "FORD JSON object 0 'returns' is neither null nor an object"
    );
    let objects = json!({"format": 1, "ford": "x", "objects": {}}).to_string();
    let error = convert(&objects, "geo").unwrap_err();
    assert!(matches!(error, FortranExtractError::Json(_)));
    assert_eq!(error.to_string(), "FORD JSON 'objects' is not a list");
}

#[test]
fn run_reports_an_unreadable_input() {
    let options = Options {
        json: "/nonexistent/fortran.json".to_owned(),
        package: "geo".to_owned(),
        out: "out".to_owned(),
    };
    let error = run(&options).unwrap_err();
    assert!(matches!(error, FortranExtractError::Read(_)));
    assert!(
        error
            .to_string()
            .starts_with("cannot read /nonexistent/fortran.json: "),
        "{error}"
    );
}

#[test]
fn parses_arguments() {
    let args: Vec<String> = ["--json", "a", "--package", "p", "--out", "o"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(parse(&args).unwrap().package, "p");
}
