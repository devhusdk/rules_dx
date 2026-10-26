use super::*;
use crate::types::render_type;
use serde_json::{json, Value};

fn settings() -> Settings {
    Settings {
        package: "geo".to_owned(),
        source_prefix: "lib".to_owned(),
        config: "tsconfig.json".to_owned(),
    }
}

fn project(children: Value) -> String {
    json!({"schemaVersion": "2.0", "variant": "project", "kind": 1, "name": "geo", "children": children}).to_string()
}

fn source(file: &str, line: u64) -> Value {
    json!([{"fileName": file, "line": line, "character": 1}])
}

fn number() -> Value {
    json!({"type": "intrinsic", "name": "number"})
}

fn scale() -> Value {
    json!({
        "id": 1, "name": "scale", "variant": "declaration", "kind": 64, "flags": {},
        "sources": source("src/a.ts", 3),
        "signatures": [{
            "id": 2, "name": "scale", "variant": "signature", "kind": 4096, "flags": {},
            "comment": {
                "summary": [{"kind": "text", "text": "Multiplies "}, {"kind": "inline-tag", "tag": "@link", "text": "x"}],
                "blockTags": [
                    {"tag": "@returns", "content": [{"kind": "text", "text": "The product."}]},
                    {"tag": "@example", "content": [{"kind": "code", "text": "```ts\nscale(1)\n```"}]},
                    {"tag": "@deprecated", "content": [{"kind": "text", "text": "Use grow."}]}
                ]
            },
            "parameters": [{
                "id": 3, "name": "by", "variant": "param", "kind": 32768,
                "flags": {"isOptional": true},
                "comment": {"summary": [{"kind": "text", "text": "The factor."}]},
                "type": number(), "defaultValue": "2"
            }],
            "type": number()
        }]
    })
}

fn unit_alias() -> Value {
    json!({
        "id": 4, "name": "Unit", "variant": "declaration", "kind": 2097152, "flags": {},
        "sources": source("src/a.ts", 1),
        "children": [{
            "id": 5, "name": "x", "variant": "declaration", "kind": 1024, "flags": {},
            "sources": source("src/a.ts", 1), "type": number()
        }]
    })
}

fn convert_ok(children: Value) -> documentation_ir::proto::DocIr {
    convert(&project(children), &settings()).expect("a valid project converts")
}

#[test]
fn a_function_becomes_a_symbol_with_signature_docs_and_source() {
    let shard = convert_ok(json!([scale()]));
    assert_eq!(shard.language, "typescript");
    assert_eq!(shard.package, "geo");
    let symbol = &shard.symbols[0];
    assert_eq!(symbol.id, "typescript:geo:scale");
    assert_eq!(
        symbol.signature_text,
        "function scale(by: number = 2): number"
    );
    assert_eq!(
        symbol.doc_markdown,
        "Multiplies `x`\n\n**Deprecated:** Use grow."
    );
    assert_eq!(symbol.params[0].name, "by");
    assert_eq!(symbol.params[0].doc, "The factor.");
    assert_eq!(
        symbol.returns.as_ref().expect("returns").doc,
        "The product."
    );
    assert_eq!(symbol.examples, vec!["```ts\nscale(1)\n```".to_owned()]);
    let source = symbol.source.as_ref().expect("a source");
    assert_eq!((source.file.as_str(), source.line), ("lib/src/a.ts", 3));
    assert_eq!(symbol.extensions[0].key, "typescript.config");
    assert_eq!(symbol.extensions[0].value, b"tsconfig.json");
}

#[test]
fn symbols_are_sorted_and_aliases_expose_their_members() {
    let shard = convert_ok(json!([scale(), unit_alias()]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "typescript:geo:Unit",
            "typescript:geo:Unit.x",
            "typescript:geo:scale"
        ]
    );
    assert_eq!(shard.symbols[0].signature_text, "type Unit = { x: number }");
    let member = &shard.symbols[1];
    assert_eq!(
        member.relations.as_ref().expect("relations").member_of,
        vec!["typescript:geo:Unit".to_owned()]
    );
}

#[test]
fn overloads_get_numbered_ids() {
    let mut first = scale();
    let second = first["signatures"][0].clone();
    first["signatures"]
        .as_array_mut()
        .expect("signatures")
        .push(second);
    let shard = convert_ok(json!([first]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["typescript:geo:scale#1", "typescript:geo:scale#2"]
    );
}

#[test]
fn private_members_are_not_documented() {
    let mut class = json!({
        "id": 9, "name": "Box", "variant": "declaration", "kind": 128, "flags": {},
        "sources": source("src/b.ts", 1),
        "children": [
            {"id": 10, "name": "hidden", "variant": "declaration", "kind": 1024,
             "flags": {"isPrivate": true}, "type": number()},
            {"id": 11, "name": "open", "variant": "declaration", "kind": 1024,
             "flags": {"isReadonly": true}, "sources": source("src/b.ts", 2), "type": number()}
        ],
        "implementedTypes": [{"type": "reference", "name": "Shape"}]
    });
    class["extendedTypes"] =
        json!([{"type": "reference", "name": "Base", "typeArguments": [number()]}]);
    let shard = convert_ok(json!([class]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["typescript:geo:Box", "typescript:geo:Box.open"]);
    assert_eq!(
        shard.symbols[0].signature_text,
        "class Box extends Base<number> implements Shape"
    );
    assert_eq!(shard.symbols[1].signature_text, "readonly open: number");
}

#[test]
fn a_re_export_points_at_its_target() {
    let alias = json!({
        "id": 20, "name": "resize", "variant": "declaration", "kind": 4194304,
        "flags": {}, "target": 1, "sources": source("src/index.ts", 1)
    });
    let shard = convert_ok(json!([scale(), alias]));
    assert!(
        shard
            .symbols
            .iter()
            .all(|s| s.id != "typescript:geo:resize"),
        "the alias has no symbol of its own; the site renders its page"
    );
    let target = shard
        .symbols
        .iter()
        .find(|s| s.id == "typescript:geo:scale")
        .expect("the target symbol");
    assert_eq!(
        target.relations.as_ref().map(|r| r.reexported_as.clone()),
        Some(vec!["typescript:geo:resize".to_owned()])
    );
}

#[test]
fn a_dangling_re_export_is_an_error() {
    let alias = json!({
        "id": 20, "name": "resize", "variant": "declaration", "kind": 4194304,
        "flags": {}, "target": 99
    });
    let error = convert(&project(json!([alias])), &settings()).expect_err("no target");
    assert!(error.contains("does not resolve"), "{error}");
}

#[test]
fn a_value_and_a_type_with_one_name_get_distinct_ids() {
    let value = json!({
        "id": 30, "name": "Color", "variant": "declaration", "kind": 32,
        "flags": {"isConst": true}, "type": number(),
        "sources": source("src/c.ts", 1)
    });
    let alias = json!({
        "id": 31, "name": "Color", "variant": "declaration", "kind": 2097152,
        "flags": {}, "type": number(), "sources": source("src/c.ts", 2)
    });
    let shard = convert_ok(json!([value, alias]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["typescript:geo:Color", "typescript:geo:Color#type"]
    );
}

#[test]
fn a_namespace_merged_with_a_value_gets_a_distinct_id() {
    let class = json!({
        "id": 40, "name": "Canvas", "variant": "declaration", "kind": 128, "flags": {},
        "sources": source("src/d.ts", 1),
        "children": [{
            "id": 41, "name": "width", "variant": "declaration", "kind": 1024, "flags": {},
            "sources": source("src/d.ts", 2), "type": number()
        }]
    });
    let namespace = json!({
        "id": 42, "name": "Canvas", "variant": "declaration", "kind": 4, "flags": {},
        "sources": source("src/d.ts", 4),
        "children": [{
            "id": 43, "name": "Options", "variant": "declaration", "kind": 2097152,
            "flags": {}, "type": number(), "sources": source("src/d.ts", 5)
        }]
    });
    let shard = convert_ok(json!([class, namespace]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "typescript:geo:Canvas",
            "typescript:geo:Canvas#namespace",
            "typescript:geo:Canvas#namespace.Options",
            "typescript:geo:Canvas.width",
        ]
    );
    let options = &shard.symbols[2];
    assert_eq!(
        options.relations.as_ref().map(|r| r.member_of.clone()),
        Some(vec!["typescript:geo:Canvas#namespace".to_owned()])
    );
}

#[test]
fn a_re_export_of_overloads_points_at_the_first_overload() {
    let mut overloaded = scale();
    let second = overloaded["signatures"][0].clone();
    overloaded["signatures"]
        .as_array_mut()
        .expect("signatures")
        .push(second);
    let alias = json!({
        "id": 20, "name": "resize", "variant": "declaration", "kind": 4194304,
        "flags": {}, "target": 1, "sources": source("src/index.ts", 1)
    });
    let shard = convert_ok(json!([overloaded, alias]));
    let first = shard
        .symbols
        .iter()
        .find(|s| s.id == "typescript:geo:scale#1")
        .expect("the first overload");
    assert_eq!(
        first.relations.as_ref().map(|r| r.reexported_as.clone()),
        Some(vec!["typescript:geo:resize".to_owned()])
    );
}

#[test]
fn interface_call_and_index_signatures_are_rendered() {
    let interface = json!({
        "id": 40, "name": "Fn", "variant": "declaration", "kind": 256, "flags": {},
        "sources": source("src/c.ts", 3),
        "signatures": [{
            "id": 41, "name": "Fn", "variant": "signature", "kind": 16384, "flags": {},
            "parameters": [{
                "id": 42, "name": "x", "variant": "param", "kind": 32768, "flags": {},
                "type": number()
            }],
            "type": {"type": "intrinsic", "name": "string"}
        }],
        "indexSignatures": [{
            "id": 43, "name": "__index", "variant": "signature", "kind": 8192, "flags": {},
            "parameters": [{
                "id": 44, "name": "key", "variant": "param", "kind": 32768, "flags": {},
                "type": {"type": "intrinsic", "name": "string"}
            }],
            "type": {"type": "intrinsic", "name": "unknown"}
        }]
    });
    let shard = convert_ok(json!([interface]));
    assert_eq!(
        shard.symbols[0].signature_text,
        "interface Fn {\n  Fn(x: number): string;\n  [key: string]: unknown;\n}"
    );
}

#[test]
fn a_wrong_or_missing_schema_version_fails() {
    let mut document: Value = serde_json::from_str(&project(json!([scale()]))).expect("json");
    document["schemaVersion"] = json!("3.0");
    let error = convert(&document.to_string(), &settings()).expect_err("wrong version");
    assert!(error.contains("schemaVersion '3.0'"), "{error}");
    document["schemaVersion"] = json!("1.0");
    let error = convert(&document.to_string(), &settings()).expect_err("old version");
    assert!(error.contains("schemaVersion '1.0'"), "{error}");
    document["schemaVersion"] = json!("2.0");
    convert(&document.to_string(), &settings()).expect("2.0 is supported");
    document
        .as_object_mut()
        .expect("object")
        .remove("schemaVersion");
    let error = convert(&document.to_string(), &settings()).expect_err("no version");
    assert!(error.contains("no schemaVersion"), "{error}");
}

#[test]
fn malformed_and_empty_extractions_fail() {
    let error = convert("{not json", &settings()).expect_err("malformed");
    assert!(error.contains("malformed"), "{error}");
    let error = convert("{\"variant\": \"declaration\"}", &settings()).expect_err("not a project");
    assert!(error.contains("not a project"), "{error}");
    let error = convert(&project(json!([])), &settings()).expect_err("empty");
    assert!(error.contains("no public symbols"), "{error}");
}

#[test]
fn unsupported_kinds_and_unsafe_sources_fail() {
    let odd = json!({"id": 1, "name": "x", "variant": "declaration", "kind": 8192, "flags": {}});
    let error = convert(&project(json!([odd])), &settings()).expect_err("unsupported kind");
    assert!(
        error.contains("unsupported TypeDoc reflection kind 8192"),
        "{error}"
    );
    let mut escaping = scale();
    escaping["signatures"][0]["sources"] = source("../outside.ts", 1);
    let error = convert(&project(json!([escaping])), &settings()).expect_err("escaping source");
    assert!(error.contains("invalid IR"), "{error}");
}

#[test]
fn duplicate_ids_fail() {
    let error = convert(&project(json!([scale(), scale()])), &settings()).expect_err("duplicate");
    assert!(error.contains("invalid IR"), "{error}");
}

fn rendered(ty: Value) -> String {
    render_type(&ty).expect("a supported type renders")
}

#[test]
fn types_render_with_the_parentheses_typescript_needs() {
    let string = json!({"type": "intrinsic", "name": "string"});
    let union = json!({"type": "union", "types": [number(), string.clone()]});
    assert_eq!(rendered(union.clone()), "number | string");
    assert_eq!(
        rendered(json!({"type": "array", "elementType": union})),
        "(number | string)[]"
    );
    assert_eq!(
        rendered(
            json!({"type": "typeOperator", "operator": "readonly", "target": {"type": "array", "elementType": number()}})
        ),
        "readonly number[]"
    );
    assert_eq!(
        rendered(
            json!({"type": "tuple", "elements": [number(), {"type": "namedTupleMember", "name": "s", "isOptional": true, "element": string}]})
        ),
        "[number, s?: string]"
    );
    assert_eq!(
        rendered(json!({"type": "literal", "value": "cm"})),
        "\"cm\""
    );
    assert_eq!(rendered(json!({"type": "literal", "value": 3})), "3");
    assert_eq!(rendered(json!({"type": "literal", "value": null})), "null");
    assert_eq!(
        rendered(json!({"type": "literal", "value": {"value": "12", "negative": true}})),
        "-12n"
    );
    assert_eq!(
        rendered(
            json!({"type": "reference", "name": "Color", "qualifiedName": "Property.Color", "package": "csstype"})
        ),
        "Property.Color"
    );
    assert_eq!(
        rendered(json!({"type": "reference", "name": "Map", "typeArguments": [number(), string]})),
        "Map<number, string>"
    );
}

#[test]
fn function_and_object_types_render() {
    let function = json!({"type": "reflection", "declaration": {"signatures": [{
        "name": "__type", "parameters": [{"name": "n", "type": number(), "flags": {}}], "type": number()
    }]}});
    assert_eq!(rendered(function.clone()), "(n: number) => number");
    assert_eq!(
        rendered(json!({"type": "array", "elementType": function})),
        "((n: number) => number)[]"
    );
    let object = json!({"type": "reflection", "declaration": {"children": [
        {"name": "a", "flags": {"isOptional": true}, "type": number()}
    ], "indexSignatures": [{"parameters": [{"name": "k", "type": {"type": "intrinsic", "name": "string"}}], "type": number()}]}});
    assert_eq!(rendered(object), "{ a?: number; [k: string]: number }");
    assert_eq!(
        rendered(json!({"type": "reflection", "declaration": {}})),
        "{}"
    );
}

#[test]
fn advanced_types_render() {
    let t = json!({"type": "reference", "name": "T"});
    assert_eq!(
        rendered(
            json!({"type": "conditional", "checkType": t, "extendsType": number(), "trueType": t, "falseType": number()})
        ),
        "T extends number ? T : number"
    );
    assert_eq!(
        rendered(
            json!({"type": "indexedAccess", "objectType": t, "indexType": {"type": "literal", "value": "a"}})
        ),
        "T[\"a\"]"
    );
    assert_eq!(
        rendered(json!({"type": "query", "queryType": {"type": "reference", "name": "x"}})),
        "typeof x"
    );
    assert_eq!(
        rendered(
            json!({"type": "predicate", "name": "v", "asserts": true, "targetType": number()})
        ),
        "asserts v is number"
    );
    assert_eq!(
        rendered(
            json!({"type": "mapped", "parameter": "K", "parameterType": t, "templateType": number(), "readonlyModifier": "+", "optionalModifier": "-"})
        ),
        "{ readonly [K in T]-?: number }"
    );
    assert_eq!(
        rendered(json!({"type": "templateLiteral", "head": "a-", "tail": [[number(), ""]]})),
        "`a-${number}`"
    );
    assert_eq!(
        rendered(json!({"type": "inferred", "name": "U", "constraint": number()})),
        "infer U extends number"
    );
    assert_eq!(
        rendered(json!({"type": "optional", "elementType": number()})),
        "number?"
    );
    assert_eq!(
        rendered(
            json!({"type": "rest", "elementType": {"type": "array", "elementType": number()}})
        ),
        "...number[]"
    );
    assert_eq!(
        rendered(
            json!({"type": "intersection", "types": [t, {"type": "union", "types": [number(), t]}]})
        ),
        "T & (number | T)"
    );
}

#[test]
fn unsupported_types_fail() {
    let error = render_type(&json!({"type": "mystery"})).expect_err("unsupported");
    assert!(
        error.contains("unsupported TypeDoc type kind 'mystery'"),
        "{error}"
    );
    let error = render_type(&json!({"type": "array"})).expect_err("missing field");
    assert!(error.contains("no 'elementType'"), "{error}");
}

#[test]
fn a_symbol_without_sources_is_an_error_naming_it() {
    let mut bare = scale();
    bare["signatures"][0]
        .as_object_mut()
        .expect("signature")
        .remove("sources");
    bare.as_object_mut().expect("node").remove("sources");
    let error = convert(&project(json!([bare])), &settings()).expect_err("no sources");
    assert!(error.contains("'scale'"), "{error}");
    assert!(error.contains("no sources"), "{error}");
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn parse_reads_one_flat_flag_set() {
    let options = parse(&args(&[
        "--typedoc",
        "a",
        "--out",
        "b",
        "--package",
        "p",
        "--source-prefix",
        "lib",
        "--config",
        "c",
    ]))
    .expect("parses");
    assert_eq!(
        options,
        Options {
            typedoc: "a".to_owned(),
            out: "b".to_owned(),
            package: "p".to_owned(),
            source_prefix: "lib".to_owned(),
            config: "c".to_owned(),
        }
    );
}

#[test]
fn parse_rejects_bad_invocations() {
    for (input, needle) in [
        (vec!["--typedoc"], "needs a value"),
        (vec!["--bogus", "x"], "unknown argument"),
        (
            vec!["--out", "b", "--package", "p"],
            "--typedoc is required",
        ),
        (
            vec![
                "--typedoc",
                "a",
                "--typedoc",
                "b",
                "--out",
                "o",
                "--package",
                "p",
            ],
            "more than once",
        ),
        (
            vec!["--typedoc", "", "--out", "o", "--package", "p"],
            "must not be empty",
        ),
    ] {
        let error = parse(&args(&input)).expect_err("rejected");
        assert!(error.contains(needle), "{input:?}: {error}");
    }
    assert!(usage().contains("typescript_extract"));
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dx-docs-ts-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

#[test]
fn run_writes_a_valid_shard_and_reports_file_errors() {
    let dir = scratch("run");
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let options = |typedoc: String, out: String| Options {
        typedoc,
        out,
        package: "geo".to_owned(),
        source_prefix: "lib".to_owned(),
        config: "cfg".to_owned(),
    };
    fs::write(path("ok.json"), project(json!([scale()]))).expect("write");
    run(&options(path("ok.json"), path("o.pb"))).expect("extracts");
    let bytes = fs::read(path("o.pb")).expect("shard");
    let shard = documentation_ir::decode_shard(&bytes).expect("a valid shard");
    assert_eq!(shard.language, "typescript");
    let error = run(&options(path("missing.json"), path("o.pb"))).expect_err("missing");
    assert!(error.contains("cannot read"), "{error}");
    fs::write(path("bad.json"), "{").expect("write");
    let error = run(&options(path("bad.json"), path("o.pb"))).expect_err("malformed");
    assert!(error.contains("malformed"), "{error}");
    let error = run(&options(path("ok.json"), path("no/such/dir/o.pb"))).expect_err("unwritable");
    assert!(error.contains("cannot write"), "{error}");
}
