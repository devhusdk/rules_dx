use super::*;
use serde_json::json;

fn item(kind: &str, name: &str) -> Value {
    json!({
        "args": "(a, b)", "description": "", "kind": kind, "line": 7, "name": name,
        "params": [{"name": "a", "type": "number", "doc": "First."}],
        "returns": [{"type": "number", "doc": "The sum."}],
        "summary": format!("Doc of {name}."), "usage": ["add(1, 2)"]
    })
}

fn document(items: Vec<Value>) -> String {
    json!({"format": 1, "ldoc": "1.5.0", "lua": "Lua 5.4", "modules": [{
        "description": "More text.", "file": "src/geo.lua", "generated": false,
        "items": Value::Array(items), "kind": "module", "line": 1, "name": "geo", "summary": "Geometry."
    }]})
    .to_string()
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == format!("lua:geo:{id}"))
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn converts_modules_functions_and_methods() {
    let json = document(vec![
        item("function", "add"),
        item("function", "geo.sub"),
        item("function", "Shape:area"),
        item("table", "defaults"),
    ]);
    let shard = convert(&json, "geo").unwrap();
    assert_eq!(shard.language, "lua");
    assert_eq!(shard.symbols.len(), 5);
    let module = symbol(&shard, "geo");
    assert_eq!(module.kind, SymbolKind::Module as i32);
    assert_eq!(module.doc_markdown, "Geometry.\n\nMore text.");
    let add = symbol(&shard, "geo.add");
    assert_eq!(add.kind, SymbolKind::Function as i32);
    assert_eq!(add.signature_text, "function geo.add(a, b)");
    assert_eq!(add.params[0].name, "a");
    assert_eq!(add.returns.as_ref().unwrap().r#type, "number");
    assert_eq!(add.examples, vec!["add(1, 2)".to_owned()]);
    assert_eq!(
        add.relations.as_ref().unwrap().member_of,
        vec!["lua:geo:geo"]
    );
    assert_eq!(symbol(&shard, "geo.sub").display_name, "sub");
    assert_eq!(
        symbol(&shard, "geo.Shape~3Aarea").kind,
        SymbolKind::Method as i32
    );
    assert_eq!(
        symbol(&shard, "geo.defaults").kind,
        SymbolKind::Variable as i32
    );
}

#[test]
fn skips_local_functions_and_marks_missing_docs() {
    let mut bare = item("function", "bare");
    bare["summary"] = json!("");
    let shard = convert(&document(vec![item("lfunction", "hidden"), bare]), "geo").unwrap();
    assert_eq!(shard.symbols.len(), 2);
    let bare = symbol(&shard, "geo.bare");
    assert!(bare.extensions.iter().any(|e| e.key == KEY_INCOMPLETE));
}

#[test]
fn rejects_unknown_formats_and_kinds() {
    let wrong = document(vec![]).replace("\"format\":1", "\"format\":2");
    let error = convert(&wrong, "geo").unwrap_err();
    assert!(matches!(
        error,
        LuaExtractError::UnsupportedFormat { found: 2 }
    ));
    assert_eq!(
        error.to_string(),
        "LDoc JSON format 2 is not supported, expected 1"
    );
    let odd = document(vec![item("spaceship", "x")]);
    let error = convert(&odd, "geo").unwrap_err();
    assert!(matches!(error, LuaExtractError::UnknownItemType { .. }));
    assert_eq!(
        error.to_string(),
        "LDoc JSON module 'geo' item 'x' has unknown item type 'spaceship'"
    );
    let error = convert("[]", "geo").unwrap_err();
    assert!(matches!(error, LuaExtractError::NotObject { .. }));
    assert_eq!(error.to_string(), "LDoc JSON is not an object");
    let error = convert(&document(vec![]), "bad package").unwrap_err();
    assert!(matches!(error, LuaExtractError::InvalidPackage { .. }));
    assert_eq!(error.to_string(), "invalid package 'bad package'");
    let error = convert(&document(vec![json!(1)]), "geo").unwrap_err();
    assert!(matches!(error, LuaExtractError::NotObject { .. }));
    assert_eq!(
        error.to_string(),
        "LDoc JSON module 'geo' 'items' entry 0 is not an object"
    );
    let generated = document(vec![]).replace("\"generated\":false", "\"generated\":1");
    let error = convert(&generated, "geo").unwrap_err();
    assert!(matches!(error, LuaExtractError::Json(_)));
    assert_eq!(
        error.to_string(),
        "LDoc JSON module 'geo' 'generated' is not a boolean"
    );
}

#[test]
fn run_reports_an_unreadable_input() {
    let options = Options {
        json: "/nonexistent/lua.json".to_owned(),
        package: "geo".to_owned(),
        out: "out".to_owned(),
    };
    let error = run(&options).unwrap_err();
    assert!(matches!(error, LuaExtractError::Read(_)));
    assert!(
        error
            .to_string()
            .starts_with("cannot read /nonexistent/lua.json: "),
        "{error}"
    );
}

#[test]
fn parse_reads_every_flag() {
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let ok = parse(&args(&["--json", "a", "--package", "p", "--out", "o"])).unwrap();
    assert_eq!(ok.package, "p");
}
