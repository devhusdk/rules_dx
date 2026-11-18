//! Renders the public API snapshot that `api/` checks in.

use std::collections::BTreeMap;

use dx_module_bazel::{ModuleFile, Value as Arg};
use dx_plan::args::{help_environment, help_surface};
use serde_json::{Value, json};

/// The files under `api/` this crate writes, in the order `update` writes them.
pub const FILES: [&str; 3] = ["cli.json", "labels.txt", "schemas.txt"];

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

/// Every file under `api/`, by name, as `update` writes it.
pub fn render() -> Result<BTreeMap<&'static str, String>, Box<dyn std::error::Error>> {
    Ok(BTreeMap::from([
        (FILES[0], cli_json()?),
        (FILES[1], labels_txt(CONFIG_BUILD)?),
        (FILES[2], schemas_txt()),
    ]))
}

/// One line per flag in `config/BUILD.bazel`: its label, rule, default (or alias target) and allowed values.
fn labels_txt(build: &str) -> Result<String, dx_module_bazel::ModuleError> {
    let file = ModuleFile::parse_build(build)?;
    let mut rows: Vec<String> = file
        .calls()
        .iter()
        .filter(|call| FLAG_RULES.contains(&call.name.as_str()))
        .filter_map(|call| {
            let name = call.str("name")?;
            let default = call
                .arg("build_setting_default")
                .or_else(|| call.arg("actual"))
                .map_or_else(String::new, literal);
            let mut row = format!("//config:{name} {} {default}", call.name);
            if let Some(values) = call.arg("values") {
                row.push(' ');
                row.push_str(&literal(values));
            }
            Some(row)
        })
        .collect();
    rows.sort();
    Ok(rows.into_iter().map(|row| row + "\n").collect())
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
    use super::{FILES, labels_txt, render, unbumped_schemas};

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
        let files = render().expect("snapshot renders");
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
            files["labels.txt"]
                .lines()
                .any(|line| line.starts_with("//config:workspace label_flag ")),
            "{}",
            files["labels.txt"]
        );
    }
}
