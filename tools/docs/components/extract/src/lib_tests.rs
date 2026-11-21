use super::*;
use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use serde_json::{Value, json};
use std::fs;

fn settings() -> Settings {
    Settings {
        language: "vue".to_owned(),
        package: "ui".to_owned(),
        source_prefix: "lib".to_owned(),
        config: "tsconfig.json".to_owned(),
    }
}

fn doc(components: Value) -> String {
    let mut document = json!({"schema": "dx-component-meta/1", "framework": "vue"});
    document["components"] = components;
    document.to_string()
}

fn button() -> Value {
    json!({
        "name": "Button", "display_name": "Button.vue", "file": "src/Button.vue", "line": 1,
        "description": "A button.",
        "props": [
            {"name": "size", "type": "number | undefined", "required": false, "default": "12",
             "description": "Size.", "tags": [{"name": "since", "text": "1.0"}], "file": "src/Button.vue", "line": 9},
            {"name": "label", "type": "string", "required": true, "default": null,
             "description": "Text.", "tags": [], "file": "src/Button.vue", "line": 6}
        ],
        "events": [{"name": "click", "type": "[id: number]", "signature": "(event: \"click\", id: number): void",
                    "description": "", "tags": [], "file": "", "line": 0}],
        "slots": [{"name": "default", "type": "{ label: string; }", "description": "Body.", "tags": [], "file": "src/Button.vue", "line": 26}],
        "exposed": [
            {"name": "focus", "type": "() => void", "callable": true, "description": "", "tags": [], "file": "src/Button.vue", "line": 37},
            {"name": "clicks", "type": "number", "description": "", "tags": [], "file": "src/Button.vue", "line": 37}
        ]
    })
}

fn convert_ok(components: Value) -> DocIr {
    let shard = convert(&doc(components), &settings()).expect("convert");
    documentation_ir::validate_shard(&shard).expect("valid");
    shard
}

fn find<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

#[test]
fn maps_every_kind_with_ids_signatures_and_membership() {
    let shard = convert_ok(json!([button()]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "vue:ui:Button",
            "vue:ui:Button.event_click",
            "vue:ui:Button.exposed_clicks",
            "vue:ui:Button.exposed_focus",
            "vue:ui:Button.prop_label",
            "vue:ui:Button.prop_size",
            "vue:ui:Button.slot_default",
        ]
    );
    let component = find(&shard, "vue:ui:Button");
    assert_eq!(component.kind, SymbolKind::Component as i32);
    assert_eq!(component.signature_text, "component Button");
    assert_eq!(component.display_name, "Button.vue");
    assert_eq!(component.doc_markdown, "A button.");
    assert_eq!(
        component.relations.as_ref().expect("relations").member_of,
        [] as [String; 0]
    );
    let kinds = |id: &str| find(&shard, id).kind;
    assert_eq!(kinds("vue:ui:Button.event_click"), SymbolKind::Event as i32);
    assert_eq!(kinds("vue:ui:Button.slot_default"), SymbolKind::Slot as i32);
    assert_eq!(
        kinds("vue:ui:Button.prop_label"),
        SymbolKind::Property as i32
    );
    assert_eq!(
        kinds("vue:ui:Button.exposed_focus"),
        SymbolKind::Method as i32
    );
    assert_eq!(
        kinds("vue:ui:Button.exposed_clicks"),
        SymbolKind::Field as i32
    );
    for symbol in shard.symbols.iter().skip(1) {
        assert_eq!(
            symbol.relations.as_ref().expect("relations").member_of,
            ["vue:ui:Button"]
        );
    }
    assert_eq!(
        find(&shard, "vue:ui:Button.event_click").signature_text,
        "(event: \"click\", id: number): void"
    );
    assert_eq!(
        find(&shard, "vue:ui:Button.slot_default").signature_text,
        "default: { label: string; }"
    );
    assert_eq!(
        find(&shard, "vue:ui:Button.prop_label").display_name,
        "Button.vue prop_label"
    );
}

#[test]
fn props_show_optional_markers_defaults_and_tags() {
    let shard = convert_ok(json!([button()]));
    let size = find(&shard, "vue:ui:Button.prop_size");
    assert_eq!(size.signature_text, "size?: number | undefined = 12");
    assert_eq!(size.doc_markdown, "Size.\n\n**@since** 1.0");
    assert_eq!(
        find(&shard, "vue:ui:Button.prop_label").signature_text,
        "label: string"
    );
}

#[test]
fn sources_get_the_prefix_and_members_without_a_file_get_none() {
    let shard = convert_ok(json!([button()]));
    let source = find(&shard, "vue:ui:Button.prop_label")
        .source
        .clone()
        .expect("source");
    assert_eq!(
        (source.file.as_str(), source.line),
        ("lib/src/Button.vue", 6)
    );
    assert!(find(&shard, "vue:ui:Button.event_click").source.is_none());
}

#[test]
fn config_is_recorded_on_every_symbol() {
    let shard = convert_ok(json!([button()]));
    for symbol in &shard.symbols {
        assert_eq!(symbol.extensions[0].key, "vue.config");
        assert_eq!(symbol.extensions[0].value, b"tsconfig.json");
    }
}

#[test]
fn components_sort_by_id_regardless_of_input_order() {
    let a = json!({"name": "Zed", "props": []});
    let b = json!({"name": "Alpha", "props": []});
    let shard = convert_ok(json!([a, b]));
    assert_eq!(shard.symbols[0].id, "vue:ui:Alpha");
    assert_eq!(shard.symbols[1].id, "vue:ui:Zed");
    assert_eq!(shard.symbols[1].display_name, "Zed");
}

fn failure(json: &str) -> ComponentExtractError {
    convert(json, &settings()).expect_err("must fail")
}

#[test]
fn rejects_malformed_or_mismatched_metadata() {
    let error = failure("{");
    assert!(matches!(error, ComponentExtractError::Malformed(_)));
    assert!(error.to_string().starts_with("invalid JSON: "), "{error}");
    let error = failure(
        &json!({"schema": "x/2", "framework": "vue", "components": [{"name": "A"}]}).to_string(),
    );
    assert!(matches!(&error, ComponentExtractError::UnsupportedSchema { found } if found == "x/2"));
    assert_eq!(
        error.to_string(),
        "unsupported schema 'x/2', expected 'dx-component-meta/1'"
    );
    let error = failure(
        &json!({"schema": SCHEMA, "framework": "svelte", "components": [{"name": "A"}]})
            .to_string(),
    );
    assert!(matches!(
        error,
        ComponentExtractError::WrongFramework { .. }
    ));
    assert_eq!(
        error.to_string(),
        "metadata is for framework 'svelte', not 'vue'"
    );
    let error = failure(&doc(json!([])));
    assert!(matches!(error, ComponentExtractError::NoComponents));
    assert_eq!(error.to_string(), "metadata has no components");
    let error = failure(&json!({"schema": SCHEMA, "framework": "vue"}).to_string());
    assert!(matches!(error, ComponentExtractError::ComponentsNotArray));
    assert_eq!(error.to_string(), "components must be an array");
}

#[test]
fn rejects_duplicates_and_unusable_names() {
    let error = failure(&doc(json!([{"name": "A"}, {"name": "A"}])));
    assert!(matches!(&error, ComponentExtractError::DuplicateComponent { name } if name == "A"));
    assert_eq!(error.to_string(), "duplicate component 'A'");
    let error = failure(&doc(json!([{"name": ""}])));
    assert!(matches!(
        error,
        ComponentExtractError::BadName {
            what: "component",
            ..
        }
    ));
    assert_eq!(
        error.to_string(),
        "component name '' is empty or has whitespace"
    );
    let error = failure(&doc(json!([{"name": "A.B"}])));
    assert!(matches!(error, ComponentExtractError::DottedName { .. }));
    assert_eq!(
        error.to_string(),
        "component name 'A.B' must not contain '.'"
    );
    let twice =
        json!({"name": "A", "props": [{"name": "x", "type": "t"}, {"name": "x", "type": "t"}]});
    let error = failure(&doc(json!([twice])));
    assert!(matches!(
        error,
        ComponentExtractError::DuplicateMember { what: "prop", .. }
    ));
    assert_eq!(error.to_string(), "duplicate prop 'x' in component 'A'");
}

#[test]
fn run_reports_an_unreadable_input() {
    let options = Options {
        meta: "/nonexistent/meta.json".to_owned(),
        out: "out".to_owned(),
        language: "vue".to_owned(),
        package: "ui".to_owned(),
        ..Options::default()
    };
    let error = run(&options).unwrap_err();
    assert!(matches!(error, ComponentExtractError::Read(_)));
    assert!(
        error
            .to_string()
            .starts_with("cannot read /nonexistent/meta.json: "),
        "{error}"
    );
}

#[test]
fn parse_reads_every_flag() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let ok = parse(&args(&[
        "--meta",
        "m",
        "--out",
        "o",
        "--language",
        "vue",
        "--package",
        "ui",
    ]))
    .expect("parse");
    assert_eq!((ok.meta.as_str(), ok.language.as_str()), ("m", "vue"));
}

#[test]
fn run_writes_nothing_on_failure() {
    let scratch = dx_test_scratch::scratch("component_extract").unwrap();
    let dir = scratch.path();
    let meta = dir.join("meta.json");
    let out = dir.join("out.pb");
    fs::write(&meta, doc(json!([]))).expect("write");
    let options = Options {
        meta: meta.display().to_string(),
        out: out.display().to_string(),
        language: "vue".to_owned(),
        package: "ui".to_owned(),
        ..Options::default()
    };
    assert!(run(&options).is_err());
    assert!(!out.exists());
    fs::write(&meta, doc(json!([button()]))).expect("write");
    run(&options).expect("run");
    let shard = documentation_ir::decode_shard(&fs::read(&out).expect("read")).expect("decode");
    assert_eq!(shard.symbols.len(), 7);
}

#[test]
fn member_and_component_names_are_escaped_in_ids() {
    let component = json!({
        "name": "My-Button",
        "events": [{"name": "update:modelValue", "type": "[value: string]"}],
        "slots": [{"name": "a/../../x", "type": "any"}],
    });
    let shard = convert_ok(json!([component]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "vue:ui:My~2DButton",
            "vue:ui:My~2DButton.event_update~3AmodelValue",
            "vue:ui:My~2DButton.slot_a~2F~2E~2E~2F~2E~2E~2Fx",
        ]
    );
    assert_eq!(
        shard.symbols[1].display_name,
        "My-Button event_update:modelValue"
    );
}
