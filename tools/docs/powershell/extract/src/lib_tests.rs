use super::*;
use documentation_ir::proto::{DocIr, Symbol};
use serde_json::{json, Value};

fn settings() -> Settings {
    Settings {
        package: "Demo".to_owned(),
        config: String::new(),
    }
}

fn param(name: &str, ty: Option<&str>, sets: Value) -> Value {
    json!({"name": name, "type": ty, "sets": sets, "aliases": []})
}

fn set(name: &str, mandatory: bool, position: Option<u32>) -> Value {
    json!({"name": name, "mandatory": mandatory, "position": position, "pipeline": false})
}

fn help(synopsis: &str, description: &str, parameters: Value) -> Value {
    json!({
        "synopsis": synopsis,
        "description": description,
        "parameters": parameters,
        "examples": [],
        "outputs": [],
        "notes": ""
    })
}

fn function(name: &str, line: u64) -> Value {
    json!({
        "name": name,
        "line": line,
        "help": help("Does it.", "", json!({})),
        "cmdlet_binding": null,
        "output_types": [],
        "aliases": [],
        "params": []
    })
}

fn file(rel: &str, functions: Vec<Value>) -> Value {
    json!({
        "rel": rel,
        "functions": functions,
        "exports": {"kind": "none", "functions": []},
        "unresolved_uses": []
    })
}

fn api(files: Vec<Value>) -> Value {
    json!({"schema": SCHEMA, "pwsh": "7.5.4", "manifest": null, "files": files})
}

fn shard(value: &Value) -> DocIr {
    convert(&value.to_string(), &settings()).expect("converts")
}

fn only(value: &Value) -> Symbol {
    let shard = shard(value);
    assert_eq!(shard.symbols.len(), 1);
    shard.symbols[0].clone()
}

fn error(value: &Value) -> String {
    convert(&value.to_string(), &settings()).expect_err("rejects")
}

fn extension(symbol: &Symbol, key: &str) -> Option<String> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| String::from_utf8(e.value.clone()).expect("utf8"))
}

#[test]
fn signature_plain_function_is_positional_without_common_parameters() {
    let mut f = function("Get-Thing", 1);
    f["params"] = json!([
        param("Name", Some("string"), json!([])),
        param("Force", Some("switch"), json!([])),
        param("Count", None, json!([])),
    ]);
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        symbol.signature_text,
        "Get-Thing [[-Name] <string>] [[-Count] <Object>] [-Force]"
    );
}

#[test]
fn signature_lists_default_set_first_then_ordinal() {
    let mut f = function("Add-Entry", 1);
    f["cmdlet_binding"] = json!({"default_set": "Entry", "positional_binding": true});
    f["params"] = json!([
        param(
            "Ledger",
            Some("object"),
            json!([set("__AllParameterSets", true, Some(0))])
        ),
        param(
            "Account",
            Some("string"),
            json!([set("Transfer", true, None)])
        ),
        param("Memo", Some("string"), json!([set("Entry", false, None)])),
        param(
            "Transfer",
            Some("switch"),
            json!([set("Transfer", true, None)])
        ),
    ]);
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        symbol.signature_text,
        "Add-Entry [-Ledger] <object> [-Memo <string>] [<CommonParameters>]\n\
         Add-Entry [-Ledger] <object> -Account <string> -Transfer [<CommonParameters>]"
    );
}

#[test]
fn signature_without_positional_binding_names_every_parameter() {
    let mut f = function("Set-Thing", 1);
    f["cmdlet_binding"] = json!({"default_set": null, "positional_binding": false});
    f["params"] = json!([param("Name", Some("string"), json!([]))]);
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        symbol.signature_text,
        "Set-Thing [-Name <string>] [<CommonParameters>]"
    );
}

#[test]
fn signature_parameter_attribute_makes_function_advanced() {
    let mut f = function("Get-Thing", 1);
    f["params"] = json!([param(
        "Name",
        Some("string"),
        json!([set("__AllParameterSets", true, None)])
    )]);
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        symbol.signature_text,
        "Get-Thing [-Name] <string> [<CommonParameters>]"
    );
}

#[test]
fn exports_all_functions_without_export_calls() {
    let shard = shard(&api(vec![
        file("a.ps1", vec![function("Get-B", 1)]),
        file("b.ps1", vec![function("Get-A", 3)]),
    ]));
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["powershell:Demo:Get-A", "powershell:Demo:Get-B"]);
    assert_eq!(shard.language, "powershell");
    assert_eq!(shard.package, "Demo");
}

#[test]
fn exports_literal_export_module_member_only() {
    let mut f = file(
        "m.psm1",
        vec![function("Get-A", 1), function("Get-Helper", 5)],
    );
    f["exports"] = json!({"kind": "literal", "functions": ["get-a"]});
    let symbol = only(&api(vec![f]));
    assert_eq!(symbol.id, "powershell:Demo:Get-A");
}

#[test]
fn exports_script_scope_function_under_its_bare_name() {
    let mut helper = function("Get-Helper", 5);
    helper["scope"] = json!("script");
    let mut hidden = function("Get-Hidden", 9);
    hidden["scope"] = json!("private");
    let symbol = only(&api(vec![file("m.psm1", vec![helper, hidden])]));
    assert_eq!(symbol.id, "powershell:Demo:Get-Helper");
}

#[test]
fn exports_skip_global_definition_named_by_export_module_member() {
    let mut global = function("Get-Foo", 1);
    global["scope"] = json!("global");
    let mut f = file("m.psm1", vec![global, function("Get-Bar", 5)]);
    f["exports"] = json!({"kind": "literal", "functions": ["Get-Foo", "Get-Bar"]});
    let symbol = only(&api(vec![f]));
    assert_eq!(symbol.id, "powershell:Demo:Get-Bar");
}

#[test]
fn exports_manifest_wildcards_ignore_case() {
    let mut value = api(vec![file(
        "m.psm1",
        vec![
            function("Get-LedgerBalance", 1),
            function("Get-Other", 2),
            function("Remove-Entry", 3),
        ],
    )]);
    value["manifest"] = json!({
        "rel": "m.psd1",
        "root_module": "m.psm1",
        "module_version": "1.0.0",
        "functions_to_export": ["get-ledger*", "Remove-Entry"],
        "aliases_to_export": null
    });
    let ids: Vec<String> = shard(&value).symbols.into_iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        [
            "powershell:Demo:Get-LedgerBalance",
            "powershell:Demo:Remove-Entry"
        ]
    );
}

#[test]
fn exports_manifest_star_keeps_every_function() {
    let mut value = api(vec![file("m.psm1", vec![function("Get-A", 1)])]);
    value["manifest"] = json!({"rel": "m.psd1", "functions_to_export": "*"});
    assert_eq!(only(&value).id, "powershell:Demo:Get-A");
}

#[test]
fn exports_reject_undefined_name() {
    let mut f = file("m.psm1", vec![function("Get-A", 1)]);
    f["exports"] = json!({"kind": "literal", "functions": ["Get-A", "Get-Missing"]});
    assert_eq!(
        error(&api(vec![f])),
        "m.psm1: exports 'Get-Missing' but no source file defines it"
    );
}

#[test]
fn exports_reject_empty_set() {
    let mut f = file("m.psm1", vec![function("Get-A", 1)]);
    f["exports"] = json!({"kind": "literal", "functions": []});
    assert_eq!(error(&api(vec![f])), "the module exports no functions");
}

#[test]
fn exports_wildcard_match_rules() {
    assert!(wildcard_match("Get-*", "get-thing"));
    assert!(wildcard_match("G?t-*g", "Get-Thing"));
    assert!(wildcard_match("*", ""));
    assert!(!wildcard_match("Get-*", "Set-Thing"));
    assert!(!wildcard_match("Get", "Get-Thing"));
}

#[test]
fn rejects_dynamic_export() {
    let mut f = file("m.psm1", vec![function("Get-A", 1)]);
    f["exports"] = json!({"kind": "dynamic", "functions": []});
    assert_eq!(
        error(&api(vec![f])),
        "m.psm1: Export-ModuleMember -Function is not a literal list"
    );
}

#[test]
fn rejects_case_duplicate() {
    let value = api(vec![file(
        "m.psm1",
        vec![function("Get-Thing", 1), function("Get-THING", 2)],
    )]);
    assert_eq!(
        error(&value),
        "m.psm1:2: function 'Get-THING' differs only by case from 'Get-Thing' at m.psm1:1"
    );
}

#[test]
fn rejects_binary_root_module() {
    let mut value = api(vec![file("m.psm1", vec![function("Get-A", 1)])]);
    value["manifest"] = json!({"rel": "m.psd1", "root_module": "Bin.DLL"});
    assert_eq!(
        error(&value),
        "m.psd1: RootModule 'Bin.DLL' is a binary module, which is not supported"
    );
}

#[test]
fn rejects_unknown_schema() {
    let mut value = api(vec![file("m.psm1", vec![function("Get-A", 1)])]);
    value["schema"] = json!("dx.powershell.api/2");
    assert_eq!(
        error(&value),
        "API JSON schema 'dx.powershell.api/2' is not supported, expected 'dx.powershell.api/1'"
    );
    let mut extra = api(vec![file("m.psm1", vec![function("Get-A", 1)])]);
    extra["surprise"] = json!(true);
    assert!(error(&extra).contains("unknown field `surprise`"));
    assert_eq!(error(&json!({"files": []})), "API JSON has no schema");
}

#[test]
fn extensions_sorted_with_aliases_config_and_incomplete() {
    let mut f = function("Add-Entry", 4);
    f["aliases"] = json!(["ae", "addent"]);
    f["params"] = json!([param("Amount", Some("int"), json!([]))]);
    let mut value = api(vec![file("m.psm1", vec![f])]);
    value["manifest"] = json!({
        "rel": "pkg/m.psd1",
        "module_version": "2.1.0",
        "aliases_to_export": ["ae"]
    });
    let symbol = only(&value);
    let keys: Vec<&str> = symbol.extensions.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "powershell.aliases",
            "powershell.config",
            "powershell.incomplete"
        ]
    );
    assert_eq!(
        extension(&symbol, "powershell.aliases").as_deref(),
        Some("ae")
    );
    assert_eq!(
        extension(&symbol, "powershell.config").as_deref(),
        Some("manifest=pkg/m.psd1;module_version=2.1.0;pwsh=7.5.4;api=dx.powershell.api/1")
    );
}

#[test]
fn extensions_sorted_config_carries_rule_config() {
    let value = api(vec![file("m.psm1", vec![function("Get-A", 1)])]);
    let symbol = convert(
        &value.to_string(),
        &Settings {
            package: "Demo".to_owned(),
            config: "flavor".to_owned(),
        },
    )
    .expect("converts")
    .symbols
    .remove(0);
    assert_eq!(
        extension(&symbol, "powershell.config").as_deref(),
        Some("manifest=;module_version=;pwsh=7.5.4;api=dx.powershell.api/1;config=flavor")
    );
    assert_eq!(extension(&symbol, "powershell.incomplete"), None);
}

#[test]
fn incomplete_names_missing_help() {
    let mut f = function("Get-A", 1);
    f["help"] = Value::Null;
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        extension(&symbol, "powershell.incomplete").as_deref(),
        Some("help")
    );
    assert_eq!(symbol.doc_markdown, "");
}

#[test]
fn incomplete_names_undocumented_params_case_insensitively() {
    let mut f = function("Get-A", 1);
    f["help"] = help("", "", json!({"NAME": "The name."}));
    f["params"] = json!([
        param("Name", None, json!([])),
        param("Count", None, json!([])),
        param("Depth", None, json!([])),
    ]);
    let symbol = only(&api(vec![file("m.psm1", vec![f])]));
    assert_eq!(
        extension(&symbol, "powershell.incomplete").as_deref(),
        Some("synopsis;params=Count,Depth")
    );
    assert_eq!(symbol.params[0].doc, "The name.");
    assert_eq!(symbol.params[1].r#type, "Object");
}

#[test]
fn crlf_help_is_normalized() {
    let mut f = function("Get-A", 7);
    f["help"] = json!({
        "synopsis": "Gets A.\r\n\r\n",
        "description": "Line one.\r\nLine two.\r\n",
        "parameters": {},
        "examples": ["Get-A\r\n\r\nGets it.\r\n"],
        "outputs": ["The A.\r\n"],
        "notes": ""
    });
    f["output_types"] = json!(["string"]);
    let symbol = only(&api(vec![file("dir/m.psm1", vec![f])]));
    assert_eq!(symbol.doc_markdown, "Gets A.\n\nLine one.\nLine two.");
    assert_eq!(symbol.examples, ["Get-A\n\nGets it."]);
    let returns = symbol.returns.expect("returns");
    assert_eq!(returns.r#type, "string");
    assert_eq!(returns.doc, "The A.");
    let source = symbol.source.expect("source");
    assert_eq!((source.file.as_str(), source.line), ("dir/m.psm1", 7));
}

#[test]
fn crlf_clean_handles_lone_carriage_returns() {
    assert_eq!(clean("  a\rb\r\n"), "a\nb");
}

fn scratch(name: &str) -> std::path::PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("powershell_extract_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn no_output_on_error() {
    let dir = scratch("error");
    let input = dir.join("api.json");
    let out = dir.join("out.ir.pb");
    let mut f = file("m.psm1", vec![function("Get-A", 1)]);
    f["exports"] = json!({"kind": "dynamic", "functions": []});
    std::fs::write(&input, api(vec![f]).to_string()).expect("write input");
    let options = Options {
        api: input.to_string_lossy().into_owned(),
        out: out.to_string_lossy().into_owned(),
        package: "Demo".to_owned(),
        config: String::new(),
    };
    assert!(run(&options).is_err());
    assert!(!out.exists());
}

#[test]
fn no_output_on_error_but_written_on_success() {
    let dir = scratch("ok");
    let input = dir.join("api.json");
    let out = dir.join("out.ir.pb");
    std::fs::write(
        &input,
        api(vec![file("m.psm1", vec![function("Get-A", 1)])]).to_string(),
    )
    .expect("write input");
    let options = Options {
        api: input.to_string_lossy().into_owned(),
        out: out.to_string_lossy().into_owned(),
        package: "Demo".to_owned(),
        config: String::new(),
    };
    run(&options).expect("runs");
    let decoded =
        documentation_ir::decode_shard(&std::fs::read(&out).expect("read")).expect("decodes");
    assert_eq!(decoded.symbols.len(), 1);
}

#[test]
fn parse_accepts_flags_and_rejects_bad_usage() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let options = parse(&args(&[
        "--api",
        "a.json",
        "--out",
        "o.pb",
        "--package",
        "P",
    ]))
    .expect("ok");
    assert_eq!(options.api, "a.json");
    assert_eq!(options.config, "");
    assert_eq!(
        parse(&args(&["--bogus", "x"])).expect_err("bad"),
        "unknown argument '--bogus'"
    );
    assert_eq!(
        parse(&args(&["--api"])).expect_err("bad"),
        "--api needs a value"
    );
    assert_eq!(
        parse(&args(&["--api", "a", "--out", "o"])).expect_err("bad"),
        "--package is required"
    );
    assert_eq!(
        parse(&args(&["--api", "", "--out", "o", "--package", "P"])).expect_err("bad"),
        "--api must not be empty"
    );
    assert_eq!(
        parse(&args(&[
            "--api",
            "a",
            "--api",
            "b",
            "--out",
            "o",
            "--package",
            "P"
        ]))
        .expect_err("bad"),
        "--api given more than once"
    );
    assert!(usage().contains("--api FILE"));
}
