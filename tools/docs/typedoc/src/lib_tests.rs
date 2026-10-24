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
    json!({"variant": "project", "kind": 1, "name": "geo", "children": children}).to_string()
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
    let resize = shard
        .symbols
        .iter()
        .find(|s| s.id == "typescript:geo:resize")
        .expect("the alias symbol");
    assert_eq!(
        resize.kind,
        documentation_ir::proto::SymbolKind::Function as i32
    );
    let extension = resize
        .extensions
        .iter()
        .find(|e| e.key == "typescript.reexport_of")
        .expect("the re-export extension");
    assert_eq!(extension.value, b"typescript:geo:scale");
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

fn shard_of(children: Value) -> documentation_ir::proto::DocIr {
    convert_ok(children)
}

#[test]
fn render_links_members_sources_and_examples() {
    let class = json!({
        "id": 9, "name": "Box", "variant": "declaration", "kind": 128, "flags": {},
        "sources": source("src/b.ts", 1),
        "children": [{"id": 11, "name": "open", "variant": "declaration", "kind": 1024,
            "flags": {}, "sources": source("src/b.ts", 2), "type": number()}]
    });
    let options = Options {
        title: "geo API".to_owned(),
        source_url: "https://example.test/blob/main/{file}#L{line}".to_owned(),
    };
    let page = render(&[shard_of(json!([scale(), class]))], &options).expect("renders");
    assert!(page.starts_with("# geo API\n"), "{page}");
    assert!(
        page.contains("<a id=\"typescript:geo:Box.open\"></a>\n### Box.open"),
        "{page}"
    );
    assert!(
        page.contains("Member of [Box](#typescript:geo:Box)"),
        "{page}"
    );
    assert!(
        page.contains("Source: [lib/src/a.ts:3](https://example.test/blob/main/lib/src/a.ts#L3)"),
        "{page}"
    );
    assert!(
        page.contains("**Parameters**\n\n- `by`: `number`. The factor."),
        "{page}"
    );
    assert!(
        page.contains("**Returns**: `number`. The product."),
        "{page}"
    );
    assert!(
        page.contains("**Example**\n\n```ts\nscale(1)\n```"),
        "{page}"
    );
    assert!(page.contains("- [scale](#typescript:geo:scale)"), "{page}");
}

#[test]
fn render_without_a_source_url_prints_the_location() {
    let options = Options {
        title: "t".to_owned(),
        source_url: String::new(),
    };
    let page = render(&[shard_of(json!([scale()]))], &options).expect("renders");
    assert!(page.contains("Source: `lib/src/a.ts:3`"), "{page}");
}

#[test]
fn render_rejects_bad_input() {
    let options = Options {
        title: "t".to_owned(),
        source_url: "https://x".to_owned(),
    };
    let error = render(&[shard_of(json!([scale()]))], &options).expect_err("no {file}");
    assert!(error.contains("{file}"), "{error}");
    let plain = Options {
        title: "t".to_owned(),
        source_url: String::new(),
    };
    let one = shard_of(json!([scale()]));
    let error = render(&[one.clone(), one], &plain).expect_err("duplicate ids");
    assert!(error.contains("duplicate symbol id"), "{error}");
    let error = render(&[], &plain).expect_err("nothing");
    assert!(error.contains("no symbols"), "{error}");
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn parse_reads_both_subcommands() {
    let ir = parse(&args(&[
        "ir",
        "--typedoc",
        "a",
        "--out",
        "b",
        "--package",
        "p",
    ]))
    .expect("ir");
    assert_eq!(
        ir,
        Command::Ir {
            typedoc: "a".to_owned(),
            out: "b".to_owned(),
            settings: Settings {
                package: "p".to_owned(),
                ..Settings::default()
            }
        }
    );
    let render = parse(&args(&[
        "render", "--shard", "a", "--shard", "b", "--out", "o", "--title", "t",
    ]))
    .expect("render");
    assert_eq!(
        render,
        Command::Render {
            shards: vec!["a".to_owned(), "b".to_owned()],
            out: "o".to_owned(),
            options: Options {
                title: "t".to_owned(),
                source_url: String::new()
            }
        }
    );
}

#[test]
fn parse_rejects_bad_invocations() {
    for (input, needle) in [
        (vec![], "subcommand is required"),
        (vec!["nope"], "unknown subcommand"),
        (vec!["ir", "--typedoc"], "needs a value"),
        (vec!["ir", "--bogus", "x"], "unknown argument"),
        (
            vec!["ir", "--out", "b", "--package", "p"],
            "--typedoc is required",
        ),
        (
            vec!["ir", "--typedoc", "", "--out", "b", "--package", "p"],
            "must not be empty",
        ),
        (
            vec![
                "ir",
                "--typedoc",
                "a",
                "--typedoc",
                "b",
                "--out",
                "b",
                "--package",
                "p",
            ],
            "more than once",
        ),
        (
            vec!["render", "--out", "o", "--title", "t"],
            "--shard is required",
        ),
    ] {
        let error = parse(&args(&input)).expect_err("rejected");
        assert!(error.contains(needle), "{input:?}: {error}");
    }
    assert!(usage().contains("docs_typedoc render"));
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dx-docs-typedoc-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

#[test]
fn run_extracts_then_renders_through_files() {
    let dir = scratch("run");
    let json = dir.join("typedoc.json");
    let shard = dir.join("shard.pb");
    let page = dir.join("api.md");
    fs::write(&json, project(json!([scale()]))).expect("write json");
    let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
    run(&Command::Ir {
        typedoc: path(&json),
        out: path(&shard),
        settings: settings(),
    })
    .expect("extracts");
    run(&Command::Render {
        shards: vec![path(&shard)],
        out: path(&page),
        options: Options {
            title: "geo".to_owned(),
            source_url: String::new(),
        },
    })
    .expect("renders");
    let text = fs::read_to_string(&page).expect("the page");
    assert!(
        text.contains("function scale(by: number = 2): number"),
        "{text}"
    );
}

#[test]
fn run_reports_missing_malformed_and_unwritable_files() {
    let dir = scratch("errors");
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let ir = |typedoc: String, out: String| Command::Ir {
        typedoc,
        out,
        settings: settings(),
    };
    let error = run(&ir(path("missing.json"), path("o.pb"))).expect_err("missing input");
    assert!(error.contains("cannot read"), "{error}");
    fs::write(path("bad.json"), "{").expect("write");
    let error = run(&ir(path("bad.json"), path("o.pb"))).expect_err("malformed");
    assert!(error.contains("malformed"), "{error}");
    fs::write(path("ok.json"), project(json!([scale()]))).expect("write");
    let error = run(&ir(path("ok.json"), path("no/such/dir/o.pb"))).expect_err("unwritable");
    assert!(error.contains("cannot write"), "{error}");
    fs::write(path("junk.pb"), "not a shard").expect("write");
    let render = |shard: String| Command::Render {
        shards: vec![shard],
        out: path("api.md"),
        options: Options {
            title: "t".to_owned(),
            source_url: String::new(),
        },
    };
    let error = run(&render(path("junk.pb"))).expect_err("junk shard");
    assert!(error.contains("is not valid IR"), "{error}");
    let error = run(&render(path("none.pb"))).expect_err("missing shard");
    assert!(error.contains("cannot read"), "{error}");
}
