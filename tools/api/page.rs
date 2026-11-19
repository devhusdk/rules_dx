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

This page lists every `dx` command, flag, argument and environment variable, every build setting, platform and \
toolchain type rules_dx makes public, and the version of each file format dx reads or writes. Use it to check what \
your scripts, CI and `BUILD` files can rely on.

This page is generated. Do not edit it by hand. Run `bazel run //tools/api:update` to refresh it.

## How to read the tables

- \"Values\" lists what a flag or argument accepts when it is a fixed set. An empty cell means free text, or a \
switch that takes no value.
- \"Default\" is the value dx uses when you leave the flag or argument out.
- \"Also\" lists the other spellings of a flag.
- A build setting is a flag you pass to Bazel as `--<setting>=<value>`. For an `alias`, \"Default\" names the \
setting it forwards to.
";

/// The module name users load rules_dx labels from.
const MODULE: &str = "@rules_dx";

/// The page for the snapshot texts `cli.json`, `labels.txt` and `schemas.txt`.
pub fn render(
    cli_json: &str,
    labels_txt: &str,
    schemas_txt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
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
    labels_sections(&mut page, labels_txt)?;
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

/// One line of `labels.txt`: a label, its rule, then for a build setting its default and allowed values.
struct LabelRow<'a> {
    label: &'a str,
    rule: &'a str,
    default: String,
    values: Vec<String>,
}

fn labels_sections(page: &mut String, labels_txt: &str) -> Result<(), String> {
    let rows = labels_txt
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(label_row)
        .collect::<Result<Vec<_>, _>>()?;
    page.push_str("\n## Build settings\n\n");
    page.push_str("| Setting | Kind | Default | Values |\n| --- | --- | --- | --- |\n");
    let mut platforms = Vec::new();
    let mut toolchain_types = Vec::new();
    for row in &rows {
        match row.rule {
            "platform" => platforms.push(absolute(row.label)),
            "toolchain_type" => toolchain_types.push(absolute(row.label)),
            _ => self::row(
                page,
                &[
                    code(&absolute(row.label)),
                    code(row.rule),
                    if row.default.is_empty() {
                        String::new()
                    } else {
                        code(&setting_value(row.label, &row.default))
                    },
                    list(&row.values, ""),
                ],
            ),
        }
    }
    page.push_str("\n## Platforms\n\nPass a platform to Bazel with `--platforms=<platform>`.\n\n");
    bullets(page, &platforms);
    page.push_str(
        "\n## Toolchain types\n\nUse a toolchain type when you register your own toolchain.\n\n",
    );
    bullets(page, &toolchain_types);
    Ok(())
}

fn label_row(line: &str) -> Result<LabelRow<'_>, String> {
    let broken = || format!("labels.txt: cannot read {line:?}");
    let (label, rest) = line.split_once(' ').ok_or_else(broken)?;
    let (rule, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let (default, rest) = take_literal(rest.trim_start()).ok_or_else(broken)?;
    let values = match rest.trim() {
        "" => Vec::new(),
        list => list
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .ok_or_else(broken)?
            .split(", ")
            .filter(|item| !item.is_empty())
            .map(|item| unquote(item).ok_or_else(broken))
            .collect::<Result<_, _>>()?,
    };
    Ok(LabelRow {
        label,
        rule,
        default,
        values,
    })
}

/// Splits one leading BUILD literal (a quoted string or a bare word) off `text`.
fn take_literal(text: &str) -> Option<(String, &str)> {
    if !text.starts_with('"') {
        let (word, rest) = text.split_once(' ').unwrap_or((text, ""));
        return Some((word.to_owned(), rest));
    }
    let mut escaped = false;
    for (index, ch) in text.char_indices().skip(1) {
        match ch {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => {
                let end = index + ch.len_utf8();
                let (literal, rest) = text.split_at_checked(end)?;
                return Some((unquote(literal)?, rest));
            }
            _ => escaped = false,
        }
    }
    None
}

/// The text inside a quoted literal, or a bare word as it is.
fn unquote(text: &str) -> Option<String> {
    if text.starts_with('"') {
        serde_json::from_str(text).ok()
    } else {
        Some(text.to_owned())
    }
}

/// `//pkg:name` as a consumer writes it.
fn absolute(label: &str) -> String {
    format!("{MODULE}{label}")
}

/// A setting's default as a consumer writes it: labels in rules_dx get the module name, other values stay as they
/// are.
fn setting_value(label: &str, value: &str) -> String {
    if value.starts_with(':') {
        let package = label.split_once(':').map_or(label, |(package, _)| package);
        return absolute(&format!("{package}{value}"));
    }
    if value.starts_with("//") {
        return absolute(value);
    }
    value.to_owned()
}

fn bullets(page: &mut String, items: &[String]) {
    for item in items {
        page.push_str(&format!("- {}\n", code(item)));
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
        let labels = concat!(
            "//config:config_tier string_flag \"standard\" [\"standard\", \"strict\"]\n",
            "//config:tool_dart label_flag \":no_dart\"\n",
            "//config:tool_ruff label_flag \"@dx_tools//:ruff\"\n",
            "//config:tool_hlint label_flag \"//haskell/private/tools/hlint/bin\"\n",
            "//config:toolchains bool_flag False\n",
            "//apple:macos_arm64 platform\n",
            "//lua:toolchain_type toolchain_type\n",
        );
        let page = render(cli, labels, "events.schema.json 4.0 sha256:aa\n").expect("page renders");
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
            "| `@rules_dx//config:config_tier` | `string_flag` | `standard` | `standard`, `strict` |",
            "| `@rules_dx//config:tool_dart` | `label_flag` | `@rules_dx//config:no_dart` |  |",
            "| `@rules_dx//config:tool_ruff` | `label_flag` | `@dx_tools//:ruff` |  |",
            "| `@rules_dx//config:tool_hlint` | `label_flag` | `@rules_dx//haskell/private/tools/hlint/bin` |  |",
            "| `@rules_dx//config:toolchains` | `bool_flag` | `False` |  |",
            "- `@rules_dx//apple:macos_arm64`",
            "- `@rules_dx//lua:toolchain_type`",
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
        assert!(render("{}", "", "").is_err());
    }

    #[test]
    fn a_broken_labels_line_is_an_error() {
        let cli = r#"{"commands":[],"environment":[]}"#;
        assert!(render(cli, "//config:x\n", "").is_err());
        assert!(render(cli, "//config:x string_flag \"open\n", "").is_err());
        assert!(render(cli, "//config:x string_flag \"a\" not-a-list\n", "").is_err());
    }
}
