//! Renders the public API snapshot that `api/` checks in.

use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{ModuleFile, Value as Arg};
use dx_plan::args::{help_environment, help_surface};
use prost::Message;
use serde_json::{Value, json};
use stardoc_output_proto::stardoc_output::{
    AttributeInfo, FunctionParamInfo, FunctionParamRole, ModuleInfo,
};

/// The files under `api/` this crate writes, in the order `update` writes them.
pub const FILES: [&str; 4] = ["cli.json", "labels.txt", "schemas.txt", "starlark.json"];

/// The rules in `config/BUILD.bazel` whose targets are public flags.
const FLAG_RULES: [&str; 5] = [
    "alias",
    "bool_flag",
    "label_flag",
    "string_flag",
    "string_list_flag",
];

const EVENTS_SCHEMA: &str = include_str!("../../cli/output/schema/events.schema.json");
const CONFIG_SCHEMA: &str = include_str!("../../cli/dx_config/schema/dx.schema.json");
const CONFIG_BUILD: &str = include_str!("../../config/BUILD.bazel");

const PUBLIC: &str = "//visibility:public";

/// The `tools/visibility` constant that names `//visibility:public`.
const PUBLIC_CONSTANT: &str = "VIS_PUBLIC";

/// Each package and the text of its `BUILD.bazel`, by the part of the API it feeds.
pub struct Builds {
    /// Language packages, for their platforms and toolchain types.
    pub langs: Vec<(String, String)>,
    /// Command packages, for every public target.
    pub commands: Vec<(String, String)>,
    /// Every package outside `private/`, `tests/` and `tools/`, for its runnable targets.
    pub packages: Vec<(String, String)>,
}

/// Every file under `api/`, by name, as `update` writes it. `starlark` holds the `starlark_doc_extract` binaryproto
/// of each public `.bzl` file and `forwards` the `streamed_jsonproto` query output of the targets macros create in
/// the consumer fixtures.
pub fn render(
    builds: &Builds,
    starlark: &[Vec<u8>],
    forwards: &[String],
) -> Result<BTreeMap<&'static str, String>, Box<dyn std::error::Error>> {
    let mut labels = labels_txt(CONFIG_BUILD)?;
    labels.push_str(&target_labels_txt(builds)?);
    labels.push_str(&hub_labels_txt(dx_families::hubs()));
    Ok(BTreeMap::from([
        (FILES[0], cli_json()?),
        (FILES[1], labels),
        (FILES[2], schemas_txt()),
        (FILES[3], starlark_json(starlark, forwards)?),
    ]))
}

/// One entry per public `.bzl` file with the signatures of its symbols: rule, macro, aspect, repository rule and tag
/// class attrs, function params and provider fields. Docstrings and attrs Bazel defines for every rule are left out.
/// Each function with a `**kwargs` param also lists, under `forwards`, the rule kinds and set attrs of the targets it
/// creates in the consumer fixtures.
fn starlark_json(
    modules: &[Vec<u8>],
    forwards: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
    let modules = modules
        .iter()
        .map(|bytes| ModuleInfo::decode(bytes.as_slice()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut forwards = forwards_by_function(forwards)?;
    let mut kwargs_modules: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut public_functions: BTreeSet<&str> = BTreeSet::new();
    for module in &modules {
        for function in &module.func_info {
            public_functions.insert(function.function_name.as_str());
            if has_kwargs(&function.parameter) {
                kwargs_modules
                    .entry(function.function_name.as_str())
                    .or_default()
                    .push(module.file.as_str());
            }
        }
    }
    for name in forwards.keys() {
        match kwargs_modules.get(name.as_str()).map(Vec::as_slice) {
            Some([_]) => {}
            // A public function without **kwargs forwards nothing a consumer picks; its params are already listed.
            None if public_functions.contains(name.as_str()) => {}
            Some(files) => {
                return Err(format!(
                    "generator_function {name} names a public function in {}",
                    files.join(", ")
                )
                .into());
            }
            None => {
                return Err(format!(
                    "generator_function {name} names no public function with a **kwargs param"
                )
                .into());
            }
        }
    }
    let mut entries = Vec::new();
    for module in modules {
        let rules = module
            .rule_info
            .iter()
            .map(|rule| {
                (
                    rule.rule_name.clone(),
                    json!({
                        "name": rule.rule_name,
                        "executable": rule.executable,
                        "test": rule.test,
                        "attrs": attrs_json(&rule.attribute),
                    }),
                )
            })
            .collect();
        let macros = module
            .macro_info
            .iter()
            .map(|item| {
                (
                    item.macro_name.clone(),
                    json!({ "name": item.macro_name, "attrs": attrs_json(&item.attribute) }),
                )
            })
            .collect();
        let functions = module
            .func_info
            .iter()
            .map(|function| {
                let mut entry = serde_json::Map::new();
                entry.insert("name".to_owned(), json!(function.function_name));
                entry.insert("params".to_owned(), json!(params_json(&function.parameter)));
                if has_kwargs(&function.parameter) {
                    let kinds = forwards.remove(&function.function_name).unwrap_or_default();
                    entry.insert(
                        "forwards".to_owned(),
                        kinds
                            .into_iter()
                            .map(|(kind, attrs)| json!({ "kind": kind, "attrs": attrs }))
                            .collect(),
                    );
                }
                (function.function_name.clone(), Value::Object(entry))
            })
            .collect();
        let providers = module
            .provider_info
            .iter()
            .map(|provider| {
                let mut fields: Vec<&str> = provider
                    .field_info
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect();
                fields.sort_unstable();
                (
                    provider.provider_name.clone(),
                    json!({ "name": provider.provider_name, "fields": fields }),
                )
            })
            .collect();
        let aspects = module
            .aspect_info
            .iter()
            .map(|aspect| {
                (
                    aspect.aspect_name.clone(),
                    json!({ "name": aspect.aspect_name, "attrs": attrs_json(&aspect.attribute) }),
                )
            })
            .collect();
        let extensions = module
            .module_extension_info
            .iter()
            .map(|extension| {
                let tag_classes = sorted_by_name(
                    extension
                        .tag_class
                        .iter()
                        .map(|tag| {
                            (
                                tag.tag_name.clone(),
                                json!({ "name": tag.tag_name, "attrs": attrs_json(&tag.attribute) }),
                            )
                        })
                        .collect(),
                );
                (
                    extension.extension_name.clone(),
                    json!({ "name": extension.extension_name, "tag_classes": tag_classes }),
                )
            })
            .collect();
        let repository_rules = module
            .repository_rule_info
            .iter()
            .map(|rule| {
                (
                    rule.rule_name.clone(),
                    json!({ "name": rule.rule_name, "attrs": attrs_json(&rule.attribute) }),
                )
            })
            .collect();
        entries.push((
            module.file.clone(),
            json!({
                "load": module.file,
                "rules": sorted_by_name(rules),
                "macros": sorted_by_name(macros),
                "functions": sorted_by_name(functions),
                "providers": sorted_by_name(providers),
                "aspects": sorted_by_name(aspects),
                "extensions": sorted_by_name(extensions),
                "repository_rules": sorted_by_name(repository_rules),
            }),
        ));
    }
    let mut text = serde_json::to_string_pretty(&sorted_by_name(entries))?;
    text.push('\n');
    Ok(text)
}

fn has_kwargs(params: &[FunctionParamInfo]) -> bool {
    params
        .iter()
        .any(|param| param.role() == FunctionParamRole::ParamRoleKwargs)
}

/// The rule kinds a macro creates, each with the attr names set on it.
type Forwards = BTreeMap<String, BTreeSet<String>>;

/// The rule kinds and explicitly set attr names of the targets each macro creates, keyed by the macro's
/// `generator_function`, from `streamed_jsonproto` query output. Hidden attrs, `name` and `generator_*` are left out.
fn forwards_by_function(
    outputs: &[String],
) -> Result<BTreeMap<String, Forwards>, Box<dyn std::error::Error>> {
    let mut forwards: BTreeMap<String, Forwards> = BTreeMap::new();
    for line in outputs.iter().flat_map(|text| text.lines()) {
        if line.trim().is_empty() {
            continue;
        }
        let target: Value = serde_json::from_str(line)?;
        let Some(rule) = target.get("rule") else {
            continue;
        };
        let attrs = rule["attribute"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let Some(function) = attrs
            .iter()
            .find(|attr| attr["name"] == "generator_function")
            .and_then(|attr| attr["stringValue"].as_str())
            .filter(|function| !function.is_empty())
        else {
            continue;
        };
        let kind = rule["ruleClass"]
            .as_str()
            .ok_or_else(|| format!("{} has no ruleClass", rule["name"]))?;
        let set = forwards
            .entry(function.to_owned())
            .or_default()
            .entry(kind.to_owned())
            .or_default();
        set.extend(
            attrs
                .iter()
                .filter(|attr| attr["explicitlySpecified"] == true)
                .filter_map(|attr| attr["name"].as_str())
                .filter(|name| {
                    !name.starts_with(['$', ':'])
                        && *name != "name"
                        && !name.starts_with("generator_")
                })
                .map(str::to_owned),
        );
    }
    Ok(forwards)
}

fn sorted_by_name(mut items: Vec<(String, Value)>) -> Vec<Value> {
    items.sort_by(|left, right| left.0.cmp(&right.0));
    items.into_iter().map(|(_, value)| value).collect()
}

fn attrs_json(attrs: &[AttributeInfo]) -> Vec<Value> {
    sorted_by_name(
        attrs
            .iter()
            .filter(|attr| !attr.natively_defined)
            .map(|attr| {
                (
                    attr.name.clone(),
                    json!({
                        "name": attr.name,
                        "type": attr.r#type().as_str_name(),
                        "default": attr.default_value,
                        "mandatory": attr.mandatory,
                    }),
                )
            })
            .collect(),
    )
}

/// Params stay in declaration order: callers may pass them by position.
fn params_json(params: &[FunctionParamInfo]) -> Vec<Value> {
    params
        .iter()
        .map(|param| {
            json!({
                "name": param.name,
                "default": param.default_value,
                "mandatory": param.mandatory,
            })
        })
        .collect()
}

/// The `$(rootpaths)` of every language `BUILD.bazel`, from `DX_API_LANG_BUILDS`.
pub fn lang_build_paths() -> Result<Vec<String>, std::env::VarError> {
    build_paths("DX_API_LANG_BUILDS")
}

/// The `$(rootpaths)` of every command package `BUILD.bazel`, from `DX_API_COMMAND_BUILDS`.
pub fn command_build_paths() -> Result<Vec<String>, std::env::VarError> {
    build_paths("DX_API_COMMAND_BUILDS")
}

/// The `$(rootpaths)` of every public package `BUILD.bazel`, from `DX_API_PACKAGE_BUILDS`.
pub fn package_build_paths() -> Result<Vec<String>, std::env::VarError> {
    build_paths("DX_API_PACKAGE_BUILDS")
}

fn build_paths(var: &str) -> Result<Vec<String>, std::env::VarError> {
    Ok(std::env::var(var)?
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}

/// The package of a `<package>/BUILD.bazel` root path; the root `BUILD.bazel` (`./BUILD.bazel` as a `$(rootpath)`)
/// is the package `""`.
pub fn package_of(path: &str) -> Result<String, String> {
    if path.strip_prefix("./").unwrap_or(path) == "BUILD.bazel" {
        return Ok(String::new());
    }
    path.strip_suffix("/BUILD.bazel")
        .map(str::to_owned)
        .ok_or_else(|| format!("{path} is not a package BUILD.bazel"))
}

/// One line per public target the API holds, with its label and rule, and for a flag its default and allowed
/// values: every named target other than a runnable one in each language and command package, and
/// `<label> runnable <rule>` for each runnable target in any public package.
fn target_labels_txt(builds: &Builds) -> Result<String, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for (package, build) in builds.langs.iter().chain(&builds.commands) {
        rows.extend(public_targets(package, build, |rule| !runnable(rule))?);
    }
    for (package, build) in &builds.packages {
        rows.extend(
            public_targets(package, build, runnable)?
                .into_iter()
                .map(|row| row.replacen(' ', " runnable ", 1)),
        );
    }
    rows.sort();
    Ok(rows.into_iter().map(|row| format!("{row}\n")).collect())
}

/// Whether `bazel run` runs targets of `rule`: every `*_binary` rule except `run_binary`, and
/// the `write_source_files` regenerators.
fn runnable(rule: &str) -> bool {
    (rule.ends_with("_binary") && rule != "run_binary")
        || matches!(rule, "write_source_files" | "write_source_file")
}

/// A row for each public named target in `build` whose rule `keep` accepts.
fn public_targets(
    package: &str,
    build: &str,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<String>, String> {
    let file = ModuleFile::parse_build(build)
        .map_err(|error| format!("//{package}:BUILD.bazel: {error}"))?;
    let calls = file.calls();
    let default_public = calls
        .iter()
        .filter(|call| call.name == "package")
        .any(|call| call.arg("default_visibility").is_some_and(lists_public));
    Ok(calls
        .iter()
        .filter(|call| keep(&call.name))
        .filter(|call| call.arg("visibility").map_or(default_public, lists_public))
        .filter_map(|call| {
            let name = call.str("name")?;
            if call.name.ends_with("_flag") {
                return Some(flag_row(package, name, call));
            }
            Some(format!("//{package}:{name} {}", call.name))
        })
        .collect())
}

/// One line per hub repository a language extension makes: its default `use_repo` name and the extension. A `*`
/// hub names one repository per dependency.
fn hub_labels_txt(hubs: &[dx_families::Hub]) -> String {
    let mut rows: Vec<String> = hubs
        .iter()
        .map(|hub| format!("@{} hub {}%{}\n", hub.hub, hub.extension, hub.ecosystem))
        .collect();
    rows.sort();
    rows.concat()
}

fn lists_public(visibility: &Arg) -> bool {
    match visibility {
        Arg::Ident(name) => name == PUBLIC_CONSTANT,
        Arg::List(items) => items
            .iter()
            .any(|item| matches!(item, Arg::Str(text) if text.value == PUBLIC)),
        Arg::Str(_) | Arg::Dict(_) | Arg::Computed { .. } => false,
    }
}

/// One line per flag in `config/BUILD.bazel`.
fn labels_txt(build: &str) -> Result<String, dx_module_bazel::ModuleError> {
    let file = ModuleFile::parse_build(build)?;
    let mut rows: Vec<String> = file
        .calls()
        .iter()
        .filter(|call| FLAG_RULES.contains(&call.name.as_str()))
        .filter_map(|call| Some(flag_row("config", call.str("name")?, call)))
        .collect();
    rows.sort();
    Ok(rows.into_iter().map(|row| format!("{row}\n")).collect())
}

/// A flag's label, rule, default (or alias target) and allowed values. Values the BUILD file computes are left out.
fn flag_row(package: &str, name: &str, call: &dx_module_bazel::Call) -> String {
    let default = call
        .arg("build_setting_default")
        .or_else(|| call.arg("actual"))
        .map_or_else(String::new, literal);
    let mut row = format!("//{package}:{name} {} {default}", call.name);
    if let Some(values @ Arg::List(_)) = call.arg("values") {
        row.push(' ');
        row.push_str(&literal(values));
    }
    row
}

/// An argument as written in the BUILD file.
fn literal(value: &Arg) -> String {
    match value {
        Arg::Str(text) => format!("{:?}", text.value),
        Arg::Ident(name) => name.clone(),
        Arg::List(items) => format!(
            "[{}]",
            items.iter().map(literal).collect::<Vec<_>>().join(", ")
        ),
        Arg::Dict(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("{}: {}", literal(key), literal(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Arg::Computed { line } => format!("<computed at line {line}>"),
    }
}

/// Each `dx` command and subcommand with its flags and positionals, their accepted values and defaults, and exit
/// codes, then the environment variables `dx --help` documents.
fn cli_json() -> Result<String, serde_json::Error> {
    let commands: Vec<Value> = help_surface()
        .into_iter()
        .map(|entry| {
            let flags: Vec<Value> = entry
                .flags
                .into_iter()
                .map(|flag| {
                    json!({
                        "name": flag.name,
                        "aliases": flag.aliases,
                        "values": flag.values,
                        "defaults": flag.defaults,
                    })
                })
                .collect();
            let positionals: Vec<Value> = entry
                .positionals
                .into_iter()
                .map(|positional| {
                    json!({
                        "name": positional.name,
                        "required": positional.required,
                        "multiple": positional.multiple,
                        "values": positional.values,
                        "defaults": positional.defaults,
                    })
                })
                .collect();
            let mut command = vec!["dx".to_owned()];
            command.extend(entry.path);
            json!({
                "command": command.join(" "),
                "flags": flags,
                "positionals": positionals,
                "exit_codes": entry.exit_codes,
            })
        })
        .collect();
    let environment: Vec<Value> = help_environment()
        .into_iter()
        .map(|env| {
            json!({
                "name": env.name,
                "shape": env.shape,
                "default_for": env.default_for,
            })
        })
        .collect();
    let mut text =
        serde_json::to_string_pretty(&json!({ "commands": commands, "environment": environment }))?;
    text.push('\n');
    Ok(text)
}

/// One line per public schema: its name, the version code declares, and the digest of its text.
pub fn schemas_txt() -> String {
    let rows = [
        (
            "events.schema.json",
            format!("{}.{}", dx_output::SCHEMA_MAJOR, dx_output::SCHEMA_MINOR),
            EVENTS_SCHEMA,
        ),
        (
            "dx.schema.json",
            format!(
                "{}.{}",
                dx_config::conventions::CURRENT_SCHEMA_VERSION,
                dx_config::conventions::SCHEMA_MINOR
            ),
            CONFIG_SCHEMA,
        ),
    ];
    rows.into_iter()
        .map(|(name, version, schema)| {
            let digest = dx_digest::sha256_hex(schema.as_bytes());
            format!("{name} {version} sha256:{digest}\n")
        })
        .collect()
}

/// Schemas whose text changed while their version stayed the same, compared with the checked-in `schemas.txt`.
#[must_use]
pub fn unbumped_schemas(checked_in: &str, current: &str) -> Vec<String> {
    let rows = |text: &str| -> BTreeMap<String, (String, String)> {
        text.lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let name = words.next()?;
                let version = words.next()?;
                let digest = words.next()?;
                Some((name.to_owned(), (version.to_owned(), digest.to_owned())))
            })
            .collect()
    };
    let before = rows(checked_in);
    rows(current)
        .into_iter()
        .filter_map(|(name, (version, digest))| match before.get(&name) {
            Some((old_version, old_digest)) if *old_version == version && *old_digest != digest => {
                Some(format!(
                    "{name} changed but its version is still {version}; bump the schema version first"
                ))
            }
            Some(_) | None => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use stardoc_output_proto::stardoc_output::{
        AttributeInfo, AttributeType, FunctionParamInfo, FunctionParamRole, MacroInfo,
        ModuleExtensionInfo, ModuleExtensionTagClassInfo, ModuleInfo, ProviderFieldInfo,
        ProviderInfo, RuleInfo, StarlarkFunctionInfo,
    };

    use super::{
        Builds, FILES, hub_labels_txt, labels_txt, package_of, render, starlark_json,
        target_labels_txt, unbumped_schemas,
    };

    fn attr(name: &str, default: &str, doc: &str) -> AttributeInfo {
        AttributeInfo {
            name: name.to_owned(),
            doc_string: doc.to_owned(),
            r#type: AttributeType::String.into(),
            default_value: default.to_owned(),
            ..AttributeInfo::default()
        }
    }

    fn module(default: &str, doc: &str) -> Vec<u8> {
        ModuleInfo {
            file: "@rules_dx//demo:defs.bzl".to_owned(),
            module_docstring: doc.to_owned(),
            rule_info: vec![RuleInfo {
                rule_name: "demo_binary".to_owned(),
                doc_string: doc.to_owned(),
                executable: true,
                attribute: vec![
                    attr("srcs", default, doc),
                    AttributeInfo {
                        natively_defined: true,
                        ..attr("tags", "[]", doc)
                    },
                ],
                ..RuleInfo::default()
            }],
            macro_info: vec![MacroInfo {
                macro_name: "demo_macro".to_owned(),
                doc_string: doc.to_owned(),
                attribute: vec![attr("mode", "\"fast\"", doc)],
                ..MacroInfo::default()
            }],
            func_info: vec![StarlarkFunctionInfo {
                function_name: "demo_test".to_owned(),
                doc_string: doc.to_owned(),
                parameter: vec![
                    FunctionParamInfo {
                        name: "name".to_owned(),
                        doc_string: doc.to_owned(),
                        mandatory: true,
                        ..FunctionParamInfo::default()
                    },
                    FunctionParamInfo {
                        name: "args".to_owned(),
                        doc_string: doc.to_owned(),
                        default_value: "[]".to_owned(),
                        ..FunctionParamInfo::default()
                    },
                    FunctionParamInfo {
                        name: "kwargs".to_owned(),
                        doc_string: doc.to_owned(),
                        role: FunctionParamRole::ParamRoleKwargs.into(),
                        ..FunctionParamInfo::default()
                    },
                ],
                ..StarlarkFunctionInfo::default()
            }],
            provider_info: vec![ProviderInfo {
                provider_name: "DemoInfo".to_owned(),
                doc_string: doc.to_owned(),
                field_info: vec![
                    ProviderFieldInfo {
                        name: "outputs".to_owned(),
                        doc_string: doc.to_owned(),
                    },
                    ProviderFieldInfo {
                        name: "inputs".to_owned(),
                        doc_string: doc.to_owned(),
                    },
                ],
                ..ProviderInfo::default()
            }],
            module_extension_info: vec![ModuleExtensionInfo {
                extension_name: "demo".to_owned(),
                doc_string: doc.to_owned(),
                tag_class: vec![ModuleExtensionTagClassInfo {
                    tag_name: "toolchain".to_owned(),
                    doc_string: doc.to_owned(),
                    attribute: vec![attr("version", default, doc)],
                }],
                ..ModuleExtensionInfo::default()
            }],
            ..ModuleInfo::default()
        }
        .encode_to_vec()
    }

    #[test]
    fn starlark_json_keeps_signatures_and_drops_docs() {
        let text = starlark_json(&[module("[]", "Old docs.")], &[]).expect("module decodes");
        assert_eq!(
            text,
            starlark_json(&[module("[]", "New docs.")], &[]).expect("module decodes")
        );
        assert_ne!(
            text,
            starlark_json(&[module("[\"a.txt\"]", "Old docs.")], &[]).expect("module decodes")
        );
        let json: serde_json::Value = serde_json::from_str(&text).expect("starlark.json parses");
        let entry = &json[0];
        assert_eq!(entry["load"], "@rules_dx//demo:defs.bzl");
        assert_eq!(
            entry["rules"][0],
            serde_json::json!({
                "name": "demo_binary",
                "executable": true,
                "test": false,
                "attrs": [{"name": "srcs", "type": "STRING", "default": "[]", "mandatory": false}],
            })
        );
        assert_eq!(entry["macros"][0]["attrs"][0]["default"], "\"fast\"");
        assert_eq!(entry["functions"][0]["params"][0]["name"], "name");
        assert_eq!(entry["functions"][0]["params"][1]["default"], "[]");
        assert_eq!(entry["functions"][0]["forwards"], serde_json::json!([]));
        assert_eq!(
            entry["providers"][0]["fields"],
            serde_json::json!(["inputs", "outputs"])
        );
        assert_eq!(
            entry["extensions"][0]["tag_classes"][0]["attrs"][0]["name"],
            "version"
        );
        assert!(!text.contains("docs"), "{text}");
    }

    fn target(function: &str, kind: &str, edition: &str) -> String {
        serde_json::json!({
            "type": "RULE",
            "rule": {
                "name": "//pkg:lib",
                "ruleClass": kind,
                "attribute": [
                    {"name": "name", "type": "STRING", "stringValue": "lib", "explicitlySpecified": true},
                    {"name": "generator_function", "type": "STRING", "stringValue": function, "explicitlySpecified": true},
                    {"name": "generator_name", "type": "STRING", "stringValue": "lib", "explicitlySpecified": true},
                    {"name": "edition", "type": "STRING", "stringValue": edition, "explicitlySpecified": true},
                    {"name": "srcs", "type": "LABEL_LIST", "explicitlySpecified": true},
                    {"name": "crate_root", "type": "LABEL", "explicitlySpecified": false},
                    {"name": "$toolchain", "type": "LABEL", "explicitlySpecified": true},
                    {"name": ":host", "type": "LABEL", "explicitlySpecified": true},
                ],
            },
        })
        .to_string()
    }

    #[test]
    fn forwards_record_each_kind_and_its_set_attrs() {
        let unrelated = serde_json::json!({
            "type": "RULE",
            "rule": {"name": "//pkg:plain", "ruleClass": "filegroup", "attribute": [
                {"name": "srcs", "type": "LABEL_LIST", "explicitlySpecified": true},
            ]},
        })
        .to_string();
        let output = [
            target("demo_test", "rust_test", "2024"),
            target("demo_test", "rust_library", "2024"),
            unrelated,
        ]
        .join("\n");
        let text = starlark_json(&[module("[]", "Docs.")], &[output]).expect("module decodes");
        let json: serde_json::Value = serde_json::from_str(&text).expect("starlark.json parses");
        assert_eq!(
            json[0]["functions"][0]["forwards"],
            serde_json::json!([
                {"kind": "rust_library", "attrs": ["edition", "srcs"]},
                {"kind": "rust_test", "attrs": ["edition", "srcs"]},
            ])
        );
        let mut changed = target("demo_test", "rust_test", "2024");
        changed = changed.replace("\"edition\"", "\"crate_name\"");
        assert_ne!(
            text,
            starlark_json(&[module("[]", "Docs.")], &[changed]).expect("module decodes")
        );
    }

    #[test]
    fn a_generator_function_without_a_public_macro_is_an_error() {
        let error = starlark_json(
            &[module("[]", "Docs.")],
            &[target("hidden_macro", "rust_test", "2024")],
        )
        .expect_err("hidden_macro is not public");
        assert!(error.to_string().contains("hidden_macro"), "{error}");
        let error = starlark_json(
            &[module("[]", "Docs."), module("[]", "Docs.")],
            &[target("demo_test", "rust_test", "2024")],
        )
        .expect_err("demo_test is in two modules");
        assert!(error.to_string().contains("demo_test"), "{error}");
    }

    #[test]
    fn a_generator_function_without_kwargs_is_skipped() {
        let mut info =
            ModuleInfo::decode(module("[]", "Docs.").as_slice()).expect("module decodes");
        for function in &mut info.func_info {
            function
                .parameter
                .retain(|param| param.role() != FunctionParamRole::ParamRoleKwargs);
        }
        let text = starlark_json(
            &[info.encode_to_vec()],
            &[target("demo_test", "rust_test", "2024")],
        )
        .expect("demo_test is public");
        let json: serde_json::Value = serde_json::from_str(&text).expect("starlark.json parses");
        assert_eq!(json[0]["functions"][0]["name"], "demo_test");
        assert!(json[0]["functions"][0].get("forwards").is_none(), "{text}");
    }

    #[test]
    fn labels_list_each_flag_with_its_default_and_values() {
        let build = r#"
load("@bazel_skylib//rules:common_settings.bzl", "bool_flag", "string_flag")

bool_flag(name = "strict", build_setting_default = True)

string_flag(name = "tier", build_setting_default = "standard", values = ["standard", "strict"])

alias(name = "clippy_flag", actual = "@rules_rust//rust/settings:clippy_flag")

filegroup(name = "not_a_flag")
"#;
        assert_eq!(
            labels_txt(build).expect("BUILD parses"),
            concat!(
                "//config:clippy_flag alias \"@rules_rust//rust/settings:clippy_flag\"\n",
                "//config:strict bool_flag True\n",
                "//config:tier string_flag \"standard\" [\"standard\", \"strict\"]\n",
            )
        );
    }

    #[test]
    fn labels_list_every_public_language_target_and_flag() {
        let open = r#"
package(default_visibility = ["//visibility:public"])

toolchain_type(name = "toolchain_type")

platform(name = "windows_arm64", constraint_values = ["@platforms//os:windows"])

filegroup(name = "srcs", visibility = ["//:__subpackages__"])

alias(name = "runtime", actual = "//open/private:runtime")

string_flag(name = "mode", build_setting_default = "fast", values = ["fast", "slow"])

string_flag(name = "compiler", build_setting_default = "llvm", values = sorted(COMPILERS))

config_setting(name = "is_fast", flag_values = {":mode": "fast"})
"#;
        let closed = r#"
package(default_visibility = ["//:__subpackages__"])

toolchain_type(name = "toolchain_type", visibility = ["//visibility:public"])

toolchain_type(name = "internal_toolchain_type")

platform(name = "hidden", visibility = ["//visibility:private"])
"#;
        let builds = Builds {
            langs: vec![
                ("open".to_owned(), open.to_owned()),
                ("closed".to_owned(), closed.to_owned()),
            ],
            commands: Vec::new(),
            packages: Vec::new(),
        };
        assert_eq!(
            target_labels_txt(&builds).expect("BUILD files parse"),
            concat!(
                "//closed:toolchain_type toolchain_type\n",
                "//open:compiler string_flag \"llvm\"\n",
                "//open:is_fast config_setting\n",
                "//open:mode string_flag \"fast\" [\"fast\", \"slow\"]\n",
                "//open:runtime alias\n",
                "//open:toolchain_type toolchain_type\n",
                "//open:windows_arm64 platform\n",
            )
        );
    }

    #[test]
    fn labels_list_public_binaries_and_command_targets() {
        let lang = r#"
package(default_visibility = ["//:__subpackages__"])

rust_binary(name = "tool", visibility = ["//visibility:public"])

run_binary(name = "generated", visibility = ["//visibility:public"])

rust_library(name = "lib", visibility = ["//visibility:public"])
"#;
        let root = r#"
package(default_visibility = ["//:__subpackages__"])

alias(name = "dx", actual = "//cli:dx", visibility = ["//visibility:public"])

filegroup(name = "module_srcs")

exports_files(["BUILD.bazel"])
"#;
        let constant = r#"
load("//tools/visibility:visibility.bzl", "VIS_PUBLIC")

package(default_visibility = VIS_PUBLIC)

rust_binary(name = "runnable")
"#;
        let private = r#"
package(default_visibility = ["//visibility:private"])

rust_binary(name = "maintainer")

alias(name = "shortcut", actual = ":maintainer")
"#;
        let root_package = package_of("BUILD.bazel").expect("root BUILD");
        let builds = Builds {
            langs: vec![("lang".to_owned(), lang.to_owned())],
            commands: vec![
                (root_package.clone(), root.to_owned()),
                ("cmd".to_owned(), constant.to_owned()),
                ("cmd/private".to_owned(), private.to_owned()),
            ],
            packages: vec![
                ("lang".to_owned(), lang.to_owned()),
                (root_package, root.to_owned()),
                ("cmd".to_owned(), constant.to_owned()),
            ],
        };
        assert_eq!(
            target_labels_txt(&builds).expect("BUILD files parse"),
            concat!(
                "//:dx alias\n",
                "//cmd:runnable runnable rust_binary\n",
                "//lang:generated run_binary\n",
                "//lang:lib rust_library\n",
                "//lang:tool runnable rust_binary\n",
            )
        );
    }

    #[test]
    fn labels_list_public_runnable_targets_of_every_package() {
        let deploy = r#"
load("//tools/visibility:visibility.bzl", "VIS_PUBLIC")

native_binary(name = "bootstrap_offline", src = ":bootstrap", visibility = VIS_PUBLIC)

sh_binary(name = "release", srcs = ["release.sh"], visibility = ["//visibility:public"])

rust_binary(name = "maintainer")

write_source_files(name = "update_docs", files = {"a": "b"}, visibility = ["//visibility:public"])

write_source_files(name = "update_private", files = {"a": "b"}, visibility = ["//visibility:private"])

run_binary(name = "generated", tool = ":maintainer", visibility = ["//visibility:public"])

filegroup(name = "srcs", visibility = ["//visibility:public"])
"#;
        let builds = Builds {
            langs: Vec::new(),
            commands: Vec::new(),
            packages: vec![("deploy".to_owned(), deploy.to_owned())],
        };
        assert_eq!(
            target_labels_txt(&builds).expect("BUILD files parse"),
            concat!(
                "//deploy:bootstrap_offline runnable native_binary\n",
                "//deploy:release runnable sh_binary\n",
                "//deploy:update_docs runnable write_source_files\n",
            )
        );
    }

    #[test]
    fn the_root_build_is_the_empty_package() {
        assert_eq!(package_of("BUILD.bazel").expect("root BUILD"), "");
        assert_eq!(package_of("./BUILD.bazel").expect("root BUILD"), "");
        assert_eq!(
            package_of("dx/tool_resolve/BUILD.bazel").expect("package BUILD"),
            "dx/tool_resolve"
        );
        assert!(package_of("dx/README.md").is_err());
    }

    #[test]
    fn labels_list_each_hub_with_its_extension() {
        let hub = |hub: &str, extension: &str, ecosystem: &str| dx_families::Hub {
            ecosystem: ecosystem.to_owned(),
            extension: extension.to_owned(),
            hub: hub.to_owned(),
            ..dx_families::Hub::default()
        };
        let hubs = [
            hub("pypi", "@rules_dx//python:extensions.bzl", "uv"),
            hub("crates", "@rules_dx//rust:extensions.bzl", "cargo"),
        ];
        assert_eq!(
            hub_labels_txt(&hubs),
            concat!(
                "@crates hub @rules_dx//rust:extensions.bzl%cargo\n",
                "@pypi hub @rules_dx//python:extensions.bzl%uv\n",
            )
        );
    }

    #[test]
    fn a_schema_change_needs_a_version_bump() {
        let before = "events.schema.json 4.0 sha256:aa\ndx.schema.json 1.0 sha256:bb\n";
        assert!(unbumped_schemas(before, before).is_empty());
        assert!(unbumped_schemas(before, "events.schema.json 4.1 sha256:cc\n").is_empty());
        assert!(unbumped_schemas(before, "dx.schema.json 1.1 sha256:cc\n").is_empty());
        assert!(unbumped_schemas(before, "new.schema.json 1 sha256:dd\n").is_empty());
        assert_eq!(
            unbumped_schemas(before, "dx.schema.json 1.0 sha256:cc\n"),
            ["dx.schema.json changed but its version is still 1.0; bump the schema version first"]
        );
    }

    #[test]
    fn the_snapshot_covers_every_file_and_flag_detail() {
        let msix = "package(default_visibility = [\"//visibility:public\"])\ntoolchain_type(name = \"toolchain_type\")\n";
        let root =
            "alias(name = \"dx\", actual = \"//cli:dx\", visibility = [\"//visibility:public\"])\n";
        let files = render(
            &Builds {
                langs: vec![("msix".to_owned(), msix.to_owned())],
                commands: vec![(String::new(), root.to_owned())],
                packages: Vec::new(),
            },
            &[module("[]", "Docs.")],
            &[],
        )
        .expect("snapshot renders");
        assert_eq!(files.keys().copied().collect::<Vec<_>>(), {
            let mut names = FILES.to_vec();
            names.sort_unstable();
            names
        });
        let cli: serde_json::Value =
            serde_json::from_str(&files["cli.json"]).expect("cli.json parses");
        let commands = cli["commands"].as_array().expect("commands list");
        assert_eq!(commands[0]["command"], "dx");
        assert!(
            commands
                .iter()
                .any(|command| command["command"] == "dx test"),
            "{cli}"
        );
        let test = commands
            .iter()
            .find(|command| command["command"] == "dx test")
            .expect("dx test entry");
        assert!(test["positionals"].as_array().is_some(), "{test}");
        let environment = cli["environment"].as_array().expect("environment list");
        assert!(
            environment
                .iter()
                .any(|env| env["name"] == "DX_OUTPUT" && env["default_for"] == "--output"),
            "{cli}"
        );
        assert!(
            environment
                .iter()
                .any(|env| env["name"] == "NO_COLOR" && env["default_for"].is_null()),
            "{cli}"
        );
        assert!(files["schemas.txt"].starts_with("events.schema.json "));
        assert!(
            files["starlark.json"].contains("\"load\": \"@rules_dx//demo:defs.bzl\""),
            "{}",
            files["starlark.json"]
        );
        assert!(
            files["labels.txt"]
                .lines()
                .any(|line| line.starts_with("//config:workspace label_flag ")),
            "{}",
            files["labels.txt"]
        );
        assert!(
            files["labels.txt"].contains("\n//:dx alias\n//msix:toolchain_type toolchain_type\n"),
            "{}",
            files["labels.txt"]
        );
        assert!(
            files["labels.txt"].ends_with("@zig hub @rules_dx//zig:extensions.bzl%zig\n"),
            "{}",
            files["labels.txt"]
        );
    }
}
