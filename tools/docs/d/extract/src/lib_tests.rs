use super::*;

const JSON: &str = r#"[{"name":"geometry","kind":"module","file":"src/geometry.d","comment":"Geometry helpers.\n","members":[
 {"name":"Circle","kind":"class","protection":"public","line":1,"comment":"A circle.\n","members":[
   {"name":"area","kind":"function","protection":"public","line":2,"comment":"Area.\n\nReturns:\n  the area\n"},
   {"name":"hidden","kind":"function","protection":"private","line":3}]},
 {"name":"add","kind":"function","protection":"public","line":4,"comment":"Adds.\n\nParams:\n    a = first\n    b = second\n\nExamples:\n---\nadd(1, 2);\n---\n","parameters":[{"name":"a"},{"name":"b","default":"2"}]},
 {"name":"add","kind":"function","protection":"public","line":6},
 {"name":"maxN","kind":"variable","protection":"public","line":7,"storageClass":["enum"]},
 {"name":"secret","kind":"variable","protection":"private","line":8}]}]"#;

const SOURCE: &str = "class Circle {\n  double area() { return 1; }\n  private void hidden() {}\nint add(int a, int b = 2) pure {\n  return a + b; }\nint add(int a);\nenum maxN = 10;\n";

fn shard(config: &str) -> DocIr {
    let sources = vec![Source {
        exec_path: "src/geometry.d".to_owned(),
        rel: "geometry/src/geometry.d".to_owned(),
        generated: false,
    }];
    let mut texts = BTreeMap::new();
    texts.insert("src/geometry.d".to_owned(), SOURCE.to_owned());
    convert(JSON, &sources, &texts, "geometry", config).expect("converts")
}

#[test]
fn ids_kinds_and_visibility() {
    let shard = shard("");
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "d:geometry:geometry",
            "d:geometry:geometry.Circle",
            "d:geometry:geometry.Circle.area",
            "d:geometry:geometry.add",
            "d:geometry:geometry.add@2",
            "d:geometry:geometry.maxN",
        ]
    );
    let kinds: Vec<i32> = shard.symbols.iter().map(|s| s.kind).collect();
    assert_eq!(kinds[2], SymbolKind::Method as i32);
    assert_eq!(kinds[5], SymbolKind::Constant as i32);
}

#[test]
fn signature_params_and_sections() {
    let shard = shard("x");
    let add = &shard.symbols[3];
    assert_eq!(add.signature_text, "int add(int a, int b = 2) pure");
    assert_eq!(add.doc_markdown, "Adds.");
    assert_eq!(add.params[0].r#type, "int");
    assert_eq!(add.params[1].doc, "second");
    assert_eq!(add.examples, ["```d\n---\nadd(1, 2);\n---\n```"]);
    assert_eq!(shard.symbols[2].returns.as_ref().expect("returns").doc, "the area");
    assert_eq!(
        shard.symbols[0].extensions[0].value,
        b"ldc-json=1;package=geometry;config=x"
    );
    assert_eq!(
        shard.symbols[3].source.as_ref().expect("source").file,
        "geometry/src/geometry.d"
    );
}

#[test]
fn unknown_module_file_is_rejected() {
    let error = convert(JSON, &[], &BTreeMap::new(), "geometry", "").expect_err("rejected");
    assert!(error.contains("not in the srcs manifest"), "{error}");
}

#[test]
fn parse_rejects_bad_arguments() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(parse(&args(&["--nope", "x"])).is_err());
    assert!(parse(&args(&["--api", "a"])).is_err());
    let all = args(&["--api", "a", "--sources", "s", "--out", "o", "--package", "p"]);
    assert_eq!(parse(&all).expect("parses").package, "p");
}

#[test]
fn declaration_stops_at_body_or_arrow() {
    assert_eq!(declaration("int f(int x) => x;\n", 1), "int f(int x)");
    assert_eq!(declaration("struct S\n{\n}\n", 1), "struct S");
}

#[test]
fn inline_returns_heading_is_a_section() {
    let sections = Sections::parse("Scales.\n\nParams:\n    v = value\n\nReturns: the scaled value\n");
    assert_eq!(sections.summary, "Scales.");
    assert_eq!(sections.returns.as_deref(), Some("the scaled value"));
}
