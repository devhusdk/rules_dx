//! Renders `docs/supported-surface.md` from the `api/` snapshot.

use serde::Deserialize;

/// The page this module writes, relative to the workspace root.
pub const PATH: &str = "docs/supported-surface.md";

#[derive(Deserialize)]
struct Cli {
    commands: Vec<Command>,
    environment: Vec<Env>,
}

#[derive(Deserialize)]
struct Command {
    command: String,
    flags: Vec<Flag>,
    positionals: Vec<Positional>,
    exit_codes: Vec<String>,
}

#[derive(Deserialize)]
struct Flag {
    name: String,
    aliases: Vec<String>,
    values: Vec<String>,
    defaults: Vec<String>,
}

#[derive(Deserialize)]
struct Positional {
    name: String,
    required: bool,
    multiple: bool,
    values: Vec<String>,
    defaults: Vec<String>,
}

#[derive(Deserialize)]
struct Env {
    name: String,
    shape: String,
    default_for: Option<String>,
}

const HEADER: &str = "# Supported surface

This page lists every `dx` command, flag, argument and environment variable, and the version of each file format \
dx reads or writes. Use it to check what your scripts and CI can rely on.

This page is generated. Do not edit it by hand. Run `bazel run //tools/api:update` to refresh it.

## How to read the tables

- \"Values\" lists what a flag or argument accepts when it is a fixed set. An empty cell means free text, or a \
switch that takes no value.
- \"Default\" is the value dx uses when you leave the flag or argument out.
- \"Also\" lists the other spellings of a flag.
";

/// The page for the snapshot texts `cli.json` and `schemas.txt`.
pub fn render(cli_json: &str, schemas_txt: &str) -> Result<String, serde_json::Error> {
    let cli: Cli = serde_json::from_str(cli_json)?;
    let mut page = String::from(HEADER);
    page.push_str("\n## Commands\n");
    for command in &cli.commands {
        command_section(&mut page, command);
    }
    page.push_str("\n## Environment variables\n\n");
    page.push_str("| Variable | Value | Default for |\n| --- | --- | --- |\n");
    for env in &cli.environment {
        let default_for = env.default_for.as_deref().map(code).unwrap_or_default();
        row(&mut page, &[code(&env.name), code(&env.shape), default_for]);
    }
    page.push_str("\n## File formats\n\n");
    page.push_str("| Schema | Version |\n| --- | --- |\n");
    for line in schemas_txt.lines() {
        let mut words = line.split_whitespace();
        if let (Some(name), Some(version)) = (words.next(), words.next()) {
            row(&mut page, &[code(name), code(version)]);
        }
    }
    Ok(page)
}

fn command_section(page: &mut String, command: &Command) {
    page.push_str(&format!("\n### `{}`\n\n", command.command));
    if !command.exit_codes.is_empty() {
        let mut codes = command.exit_codes.clone();
        codes.sort_unstable();
        page.push_str(&format!("Exit codes: {}.\n\n", list(&codes, "")));
    }
    if !command.positionals.is_empty() {
        page.push_str("| Argument | Required | Repeats | Values | Default |\n| --- | --- | --- | --- | --- |\n");
        for positional in &command.positionals {
            row(
                page,
                &[
                    code(&positional.name),
                    yes_no(positional.required),
                    yes_no(positional.multiple),
                    list(&positional.values, ""),
                    list(&positional.defaults, ""),
                ],
            );
        }
        page.push('\n');
    }
    if command.flags.is_empty() {
        page.push_str("No flags.\n");
        return;
    }
    page.push_str("| Flag | Also | Values | Default |\n| --- | --- | --- | --- |\n");
    for flag in &command.flags {
        row(
            page,
            &[
                code(&flag.name),
                list(&flag.aliases, ""),
                list(&flag.values, ""),
                list(&flag.defaults, ""),
            ],
        );
    }
}

fn row(page: &mut String, cells: &[String]) {
    page.push_str(&format!("| {} |\n", cells.join(" | ")));
}

/// A code span that is safe inside a table cell.
fn code(text: &str) -> String {
    format!("`{}`", text.replace('|', "\\|"))
}

fn list(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        return empty.to_owned();
    }
    items
        .iter()
        .map(|item| code(item))
        .collect::<Vec<_>>()
        .join(", ")
}

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn the_page_lists_commands_flags_environment_and_formats() {
        let cli = r#"{"commands":[{"command":"dx test","exit_codes":["2","0"],
            "flags":[{"name":"--output","aliases":["-o"],"values":["human","json"],"defaults":["human"]},
                     {"name":"--base","aliases":[],"values":[],"defaults":[]}],
            "positionals":[{"name":"TARGETS","required":false,"multiple":true,"values":[],"defaults":[]}]},
            {"command":"dx clean","exit_codes":[],"flags":[],"positionals":[]}],
            "environment":[{"name":"DX_BUILD_SLOTS","shape":"auto|n|off","default_for":"--build-slots"},
                           {"name":"NO_COLOR","shape":"any","default_for":null}]}"#;
        let page = render(cli, "events.schema.json 4.0 sha256:aa\n").expect("page renders");
        for line in [
            "### `dx test`",
            "Exit codes: `0`, `2`.",
            "| `TARGETS` | no | yes |  |  |",
            "| `--output` | `-o` | `human`, `json` | `human` |",
            "| `--base` |  |  |  |",
            "### `dx clean`",
            "No flags.",
            "| `DX_BUILD_SLOTS` | `auto\\|n\\|off` | `--build-slots` |",
            "| `NO_COLOR` | `any` |  |",
            "| `events.schema.json` | `4.0` |",
        ] {
            assert!(
                page.lines().any(|got| got == line),
                "missing {line:?} in\n{page}"
            );
        }
    }

    #[test]
    fn a_broken_snapshot_is_an_error() {
        assert!(render("{}", "").is_err());
    }
}
