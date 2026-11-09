use super::*;
use serde_json::json;

const P: &str = "php:geo:";

fn element(kind: &str, fqsen: &str, name: &str, parent: &str) -> Value {
    json!({
        "type": kind, "fqsen": fqsen, "name": name, "parent": parent,
        "file": "src/Geo.php", "generated": false, "line": 3,
        "summary": format!("Doc of {name}."), "description": "", "tags": [],
        "visibility": "public", "abstract": false, "final": false, "static": false,
        "readonly": false, "superclass": "", "interfaces": [], "traits": [],
        "params": [], "return_type": "", "value_type": "", "value": ""
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value[key] = field;
    value
}

fn tag(tag: &str, name: &str, types: &str, text: &str) -> Value {
    json!({"tag": tag, "name": name, "type": types, "text": text})
}

fn document(objects: Vec<Value>) -> Value {
    json!({"format": 1, "php": "8.5.12", "phpdocumentor": "3.10.0", "objects": objects})
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

fn extension_of<'a>(symbol: &'a Symbol, key: &str) -> Option<&'a str> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .and_then(|e| std::str::from_utf8(&e.value).ok())
}

fn strings_of(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parse_accepts_and_rejects_arguments() {
    let args = strings_of(&["--json", "a", "--package", "p", "--out", "o"]);
    let options = parse(&args).unwrap();
    assert_eq!(options.json, "a");
    assert_eq!(options.package, "p");
    assert_eq!(options.out, "o");
    assert!(parse(&strings_of(&["--nope", "x"])).unwrap_err().contains("unknown"));
    assert!(parse(&strings_of(&["--json"])).unwrap_err().contains("needs a value"));
    assert!(
        parse(&strings_of(&["--json", "a", "--json", "b"]))
            .unwrap_err()
            .contains("more than once")
    );
    assert!(parse(&strings_of(&["--json", "a"])).unwrap_err().contains("required"));
}

#[test]
fn namespaces_become_dotted_escaped_ids() {
    let shard = convert_all(vec![
        element("class", "\\Geo\\Circle", "Circle", ""),
        element("function", "\\Geo\\total()", "total", ""),
    ]);
    assert_eq!(ids(&shard), vec!["php:geo:Geo.Circle", "php:geo:Geo.total"]);
    assert_eq!(symbol(&shard, "Geo.Circle").display_name, "Geo\\Circle");
}

#[test]
fn kinds_signatures_and_members() {
    let class = with(
        with(element("class", "Geo\\Circle", "Circle", ""), "final", json!(true)),
        "interfaces",
        json!(["Geo\\Shape"]),
    );
    let method = with(
        with(
            element("method", "Geo\\Circle::scale()", "scale", "Geo\\Circle"),
            "params",
            json!([{"name": "factor", "type": "float", "default": "1.0", "variadic": false, "by_ref": false}]),
        ),
        "return_type",
        json!("self"),
    );
    let property = with(
        element("property", "Geo\\Circle::$radius", "radius", "Geo\\Circle"),
        "value_type",
        json!("float"),
    );
    let constant = with(
        element("constant", "Geo\\Circle::PI", "PI", "Geo\\Circle"),
        "value",
        json!("3.14"),
    );
    let shard = convert_all(vec![
        class,
        element("interface", "Geo\\Shape", "Shape", ""),
        method,
        property,
        constant,
    ]);
    let class = symbol(&shard, "Geo.Circle");
    assert_eq!(class.kind, SymbolKind::Class as i32);
    assert_eq!(class.signature_text, "final class Geo\\Circle implements \\Geo\\Shape");
    let relations = class.relations.as_ref().unwrap();
    assert_eq!(relations.implements, vec![format!("{P}Geo.Shape")]);
    assert_eq!(symbol(&shard, "Geo.Shape").kind, SymbolKind::Interface as i32);
    let method = symbol(&shard, "Geo.Circle.scale");
    assert_eq!(method.kind, SymbolKind::Method as i32);
    assert_eq!(
        method.signature_text,
        "public function scale(float $factor = 1.0): self"
    );
    assert_eq!(method.relations.as_ref().unwrap().member_of, vec![format!("{P}Geo.Circle")]);
    let property = symbol(&shard, "Geo.Circle.~24radius");
    assert_eq!(property.kind, SymbolKind::Property as i32);
    assert_eq!(property.signature_text, "public float $radius");
    assert_eq!(property.display_name, "Geo\\Circle::$radius");
    let constant = symbol(&shard, "Geo.Circle.PI");
    assert_eq!(constant.kind, SymbolKind::Constant as i32);
    assert_eq!(constant.signature_text, "public const PI = 3.14");
}

#[test]
fn enums_list_their_cases() {
    let unit = with(element("enum", "Geo\\Unit", "Unit", ""), "value_type", json!("string"));
    let case = with(element("case", "Geo\\Unit::Metre", "Metre", "Geo\\Unit"), "value", json!("'m'"));
    let shard = convert_all(vec![unit, case]);
    assert_eq!(symbol(&shard, "Geo.Unit").signature_text, "enum Geo\\Unit: string");
    let case = symbol(&shard, "Geo.Unit.Metre");
    assert_eq!(case.kind, SymbolKind::EnumVariant as i32);
    assert_eq!(case.signature_text, "case Metre = 'm'");
}

#[test]
fn hidden_elements_are_dropped() {
    let mut private = element("method", "Geo\\Circle::a()", "a", "Geo\\Circle");
    private["visibility"] = json!("private");
    let internal = with(
        element("method", "Geo\\Circle::b()", "b", "Geo\\Circle"),
        "tags",
        json!([tag("internal", "", "", "")]),
    );
    let hidden_class = with(
        element("class", "Geo\\Hidden", "Hidden", ""),
        "tags",
        json!([tag("ignore", "", "", "")]),
    );
    let member = element("method", "Geo\\Hidden::c()", "c", "Geo\\Hidden");
    let shard = convert_all(vec![
        element("class", "Geo\\Circle", "Circle", ""),
        private,
        internal,
        hidden_class,
        member,
        element("method", "Geo\\Circle::d()", "d", "Geo\\Circle"),
    ]);
    assert_eq!(ids(&shard), vec!["php:geo:Geo.Circle", "php:geo:Geo.Circle.d"]);
}

#[test]
fn docs_params_and_returns_are_read_from_tags() {
    let function = with(
        with(
            with(
                element("function", "Geo\\area()", "area", ""),
                "params",
                json!([{"name": "r", "type": "", "default": "", "variadic": false, "by_ref": false}]),
            ),
            "tags",
            json!([tag("param", "r", "float", "The radius."), tag("return", "", "float", "The area.")]),
        ),
        "description",
        json!("Longer text.\r\n"),
    );
    let shard = convert_all(vec![function]);
    let function = symbol(&shard, "Geo.area");
    assert_eq!(function.doc_markdown, "Doc of area.\n\nLonger text.");
    assert_eq!(function.params[0].r#type, "float");
    assert_eq!(function.params[0].doc, "The radius.");
    let returns = function.returns.as_ref().unwrap();
    assert_eq!(returns.r#type, "float");
    assert_eq!(returns.doc, "The area.");
    assert_eq!(extension_of(function, KEY_INCOMPLETE), None);
    assert_eq!(
        extension_of(function, KEY_CONFIG),
        Some("php=8.5.12;phpdocumentor=3.10.0;format=1")
    );
}

#[test]
fn missing_docs_are_marked_incomplete() {
    let mut function = element("function", "Geo\\bare()", "bare", "");
    function["summary"] = json!("");
    function["params"] =
        json!([{"name": "x", "type": "", "default": "", "variadic": false, "by_ref": false}]);
    let shard = convert_all(vec![function]);
    assert_eq!(
        extension_of(symbol(&shard, "Geo.bare"), KEY_INCOMPLETE),
        Some("doc;params=x;param_types=x;return_type")
    );
}

#[test]
fn constructors_need_no_return() {
    let ctor = element("method", "Geo\\C::__construct()", "__construct", "Geo\\C");
    let shard = convert_all(vec![element("class", "Geo\\C", "C", ""), ctor]);
    let ctor = symbol(&shard, "Geo.C.__construct");
    assert!(ctor.returns.is_none());
    assert_eq!(extension_of(ctor, KEY_INCOMPLETE), None);
}

#[test]
fn rejects_malformed_input() {
    assert!(convert("{", "geo").unwrap_err().contains("malformed"));
    assert!(convert("[]", "geo").unwrap_err().contains("not an object"));
    let wrong = json!({"format": 2, "php": "", "phpdocumentor": "", "objects": []});
    assert!(convert(&wrong.to_string(), "geo").unwrap_err().contains("not supported"));
    assert!(convert(&document(vec![]).to_string(), "bad pkg").unwrap_err().contains("invalid package"));
    assert!(convert(&document(vec![]).to_string(), "geo").unwrap_err().contains("no public PHP API"));
    let unknown = document(vec![element("alien", "A", "A", "")]);
    assert!(convert(&unknown.to_string(), "geo").unwrap_err().contains("unknown type"));
}
