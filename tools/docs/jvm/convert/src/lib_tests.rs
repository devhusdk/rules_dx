use super::*;
use serde_json::json;

const MANIFEST: &str = "ext/a/Circle.java\ta/Circle.java\t0\nbazel-out/gen/G.java\tgen/G.java\t1\n";

fn sym(qualified: &str, kind: &str, overload: Value, doc: &str) -> Value {
    json!({
        "qualified": qualified, "overload": overload, "kind": kind,
        "signature": format!("sig {qualified}"), "doc": doc,
        "params": [], "returns": null, "file": "ext/a/Circle.java", "line": 7,
        "member_of": null, "implements": []
    })
}

fn doc(symbols: Value) -> Value {
    json!({
        "format": 1, "language": "java", "tool": "javadoc", "tool_version": "21",
        "symbols": symbols
    })
}

fn full() -> Value {
    let mut circle = sym(
        "a.Circle",
        "class",
        Value::Null,
        "A [`Shape`](jvm:a.Shape).",
    );
    circle["implements"] = json!(["a.Shape", "java.io.Serializable"]);
    let mut scale1 = sym("a.Circle.scale", "method", json!(["double"]), "One.");
    scale1["member_of"] = json!("a.Circle");
    let mut scale2 = sym(
        "a.Circle.scale",
        "method",
        json!(["double", "double"]),
        "Two.",
    );
    scale2["member_of"] = json!("a.Circle");
    let mut ctor = sym(
        "a.Circle.Circle",
        "constructor",
        json!([]),
        "[x](jvm:a.Circle.scale(double,double)) [y](jvm:java.lang.String)",
    );
    ctor["member_of"] = json!("a.Circle");
    doc(json!([
        sym("a", "package", Value::Null, "Pkg."),
        circle,
        sym("a.Shape", "interface", Value::Null, ""),
        scale1,
        scale2,
        ctor,
    ]))
}

fn run_convert(value: &Value) -> Result<DocIr, String> {
    convert(&value.to_string(), "java", "demo", MANIFEST, &[])
}

fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

#[test]
fn ids_escape_each_segment_and_the_overload_suffix() {
    let shard = run_convert(&full()).unwrap();
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"java:demo:a"));
    assert!(ids.contains(&"java:demo:a.Circle.scale~28double~29"));
    let two = symbol(&shard, "java:demo:a.Circle.scale~28double~2Cdouble~29");
    assert_eq!(two.display_name, "a.Circle.scale(double, double)");
    assert_eq!(two.kind, SymbolKind::Method as i32);
    assert!(ids.contains(&"java:demo:a.Circle.Circle~28~29"));
    assert_eq!(id_of("p:", "a.B$C.\u{e9}", None), "p:a.B~24C.~C3~A9");
}

#[test]
fn relations_and_links_resolve_in_shard_and_drop_the_jdk() {
    let shard = run_convert(&full()).unwrap();
    let circle = symbol(&shard, "java:demo:a.Circle");
    let relations = circle.relations.as_ref().unwrap();
    assert_eq!(relations.implements, vec!["java:demo:a.Shape".to_owned()]);
    assert_eq!(circle.doc_markdown, "A [Shape](dx:java:demo:a.Shape).");
    let ctor = symbol(&shard, "java:demo:a.Circle.Circle~28~29");
    assert_eq!(
        ctor.relations.as_ref().unwrap().member_of,
        vec!["java:demo:a.Circle".to_owned()]
    );
    assert_eq!(
        ctor.doc_markdown,
        "[x](dx:java:demo:a.Circle.scale~28double~2Cdouble~29) `y`"
    );
}

#[test]
fn dep_shards_resolve_by_display_name_and_overload() {
    let dep = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "java".to_owned(),
        package: "shapes".to_owned(),
        symbols: vec![Symbol {
            id: "java:shapes:b.Shape".to_owned(),
            ..Symbol::default()
        }],
    };
    let mut item = sym("a.Circle", "class", Value::Null, "[S](jvm:b.Shape)");
    item["implements"] = json!(["b.Shape"]);
    let shard = convert(
        &doc(json!([item])).to_string(),
        "java",
        "demo",
        MANIFEST,
        &[dep],
    )
    .unwrap();
    let circle = symbol(&shard, "java:demo:a.Circle");
    assert_eq!(circle.doc_markdown, "[S](dx:java:shapes:b.Shape)");
    assert_eq!(
        circle.relations.as_ref().unwrap().implements,
        vec!["java:shapes:b.Shape".to_owned()]
    );
}

#[test]
fn generated_sources_keep_their_flag_and_repo_relative_path() {
    let mut item = sym("a.G", "class", Value::Null, "");
    item["file"] = json!("bazel-out/gen/G.java");
    let shard = run_convert(&doc(json!([item]))).unwrap();
    let source = symbol(&shard, "java:demo:a.G").source.clone().unwrap();
    assert_eq!(
        (source.file.as_str(), source.generated),
        ("gen/G.java", true)
    );
}

#[test]
fn bad_inputs_are_rejected() {
    let reject = |value: Value, needle: &str| {
        let error = run_convert(&value).expect_err(needle);
        assert!(error.contains(needle), "{error}");
    };
    let one = |item: Value| doc(json!([item]));
    let mut wrong_format = full();
    wrong_format["format"] = json!(2);
    reject(wrong_format, "format");
    let mut wrong_language = full();
    wrong_language["language"] = json!("kotlin");
    reject(wrong_language, "language");
    let mut foreign = sym("a.X", "class", Value::Null, "");
    foreign["file"] = json!("nope/X.java");
    reject(one(foreign), "not in the srcs manifest");
    let mut zero = sym("a.X", "class", Value::Null, "");
    zero["line"] = json!(0);
    reject(one(zero), "line");
    reject(
        one(sym("a.X", "object", Value::Null, "")),
        "not valid for java",
    );
    reject(
        one(sym("a.X", "trait", Value::Null, "")),
        "not valid for java",
    );
    reject(one(sym("a.X", "widget", Value::Null, "")), "unknown kind");
    reject(one(sym("a.X", "method", Value::Null, "")), "overload");
    reject(doc(json!([])), "no public symbols");
    let twice = doc(json!([
        sym("a.X", "class", Value::Null, ""),
        sym("a.X", "interface", Value::Null, "")
    ]));
    reject(twice, "duplicate id");
    assert!(convert("{", "java", "demo", MANIFEST, &[]).is_err());
}

#[test]
fn output_is_deterministic() {
    let a = documentation_ir::encode_shard(&run_convert(&full()).unwrap()).unwrap();
    let b = documentation_ir::encode_shard(&run_convert(&full()).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn parse_requires_every_flag() {
    let args: Vec<String> = [
        "--json",
        "a",
        "--language",
        "java",
        "--package",
        "p",
        "--srcs-manifest",
        "m",
        "--dep-shard",
        "d1",
        "--dep-shard",
        "d2",
        "--out",
        "o",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    assert_eq!(parse(&args).unwrap().dep_shards, vec!["d1", "d2"]);
    assert!(parse(&args[..4]).is_err());
}

fn kotlin_doc(symbols: Value) -> Value {
    let mut value = doc(symbols);
    value["language"] = json!("kotlin");
    value["tool"] = json!("dokka");
    value
}

fn run_kotlin(value: &Value) -> Result<DocIr, String> {
    convert(&value.to_string(), "kotlin", "demo", MANIFEST, &[])
}

#[test]
fn kotlin_kinds_and_receiver_overload_ids() {
    let mut grow = sym("a.grow", "function", json!(["a.Circle", "double"]), "");
    grow["overload"] = json!(["Circle", "Double"]);
    let mut companion = sym("a.Circle.Companion", "object", Value::Null, "");
    companion["member_of"] = json!("a.Circle");
    let shard = run_kotlin(&kotlin_doc(json!([
        sym("a.Circle", "class", Value::Null, ""),
        companion,
        sym("a.Alias", "typealias", Value::Null, ""),
        sym("a.size", "property", Value::Null, ""),
        grow,
    ])))
    .expect("kotlin converts");
    let kinds: Vec<_> = shard.symbols.iter().map(|s| s.kind()).collect();
    assert!(kinds.contains(&SymbolKind::TypeAlias));
    assert!(kinds.contains(&SymbolKind::Property));
    assert!(kinds.contains(&SymbolKind::Function));
    let grow = shard
        .symbols
        .iter()
        .find(|s| s.id.contains("grow"))
        .expect("grow");
    assert!(grow.id.contains("Circle"), "{}", grow.id);
}

#[test]
fn kotlin_rejects_foreign_kinds_and_duplicate_overloads() {
    let one = |item: Value| kotlin_doc(json!([item]));
    let error = run_kotlin(&one(sym("a.X", "trait", Value::Null, ""))).expect_err("trait");
    assert!(error.contains("not valid for kotlin"), "{error}");
    let error = run_kotlin(&one(sym("a.X", "method", json!([]), ""))).expect_err("method");
    assert!(error.contains("not valid for kotlin"), "{error}");
    let twice = kotlin_doc(json!([
        sym("a.f", "function", json!(["Int"]), ""),
        sym("a.f", "function", json!(["Int"]), "")
    ]));
    let error = run_kotlin(&twice).expect_err("duplicate");
    assert!(error.contains("duplicate id"), "{error}");
}
