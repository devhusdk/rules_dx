use serde::Deserialize;

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

#[derive(Deserialize)]
struct Module {
    load: String,
    rules: Vec<Rule>,
    macros: Vec<Named>,
    functions: Vec<Function>,
    providers: Vec<Provider>,
    aspects: Vec<Named>,
    extensions: Vec<Extension>,
    repository_rules: Vec<Named>,
}

#[derive(Deserialize)]
struct Rule {
    name: String,
    executable: bool,
    test: bool,
    attrs: Vec<Attr>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
    attrs: Vec<Attr>,
}

#[derive(Deserialize)]
struct Attr {
    name: String,
    r#type: String,
    default: String,
    mandatory: bool,
}

#[derive(Deserialize)]
struct Function {
    name: String,
    params: Vec<Param>,
}

#[derive(Deserialize)]
struct Param {
    name: String,
    default: String,
    mandatory: bool,
}

#[derive(Deserialize)]
struct Provider {
    name: String,
    fields: Vec<String>,
}

#[derive(Deserialize)]
struct Extension {
    name: String,
    tag_classes: Vec<Named>,
}

const HEADER: &str = "# Supported surface

This page lists every `dx` command, flag, argument and environment variable, every `.bzl` file you can load with \
its rules, macros, attributes, providers and module extensions, every build setting, platform, toolchain type, hub \
repository and target rules_dx makes public, and the version of each file format dx reads or writes. Use it to \
check what your scripts, CI, `MODULE.bazel` and `BUILD` files can rely on.

This page is generated. Do not edit it by hand. Run `bazel run //tools/api:update` to refresh it.

## How to read the tables

- \"Values\" lists what a flag or argument accepts when it is a fixed set. An empty cell means free text, or a \
switch that takes no value.
- \"Default\" is the value dx uses when you leave the flag or argument out.
- \"Also\" lists the other spellings of a flag.
- A build setting is a flag you pass to Bazel as `--<setting>=<value>`. For an `alias`, \"Default\" names the \
setting it forwards to.
- `pinned` in \"Default\" means the tool rules_dx pins. Set the flag or argument to your own label to replace it.
- An attribute is a named argument of a rule, macro or tag. \"Required\" says whether you must set it.
";

const MODULE: &str = "@rules_dx";

pub fn render(
    cli_json: &str,
    starlark_json: &str,
    labels_txt: &str,
    schemas_txt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cli: Cli = serde_json::from_str(cli_json)?;
    let modules: Vec<Module> = serde_json::from_str(starlark_json)?;
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
    starlark_sections(&mut page, &modules);
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

fn starlark_sections(page: &mut String, modules: &[Module]) {
    page.push_str("\n## Starlark files\n\nLoad a symbol in a `BUILD` or `.bzl` file with `load(\"<file>\", \"<symbol>\")`. \
Use a module extension in `MODULE.bazel` with `use_extension(\"<file>\", \"<extension>\")`.\n");
    for module in modules {
        page.push_str(&format!("\n### `{}`\n", module.load));
        for rule in &module.rules {
            let kind = match (rule.test, rule.executable) {
                (true, _) => "Test rule. Run it with `bazel test`.",
                (false, true) => "Rule. Run it with `bazel run`.",
                (false, false) => "Rule.",
            };
            symbol_heading(page, &rule.name, kind);
            attrs_table(page, &rule.attrs);
        }
        for item in &module.macros {
            symbol_heading(page, &item.name, "Macro.");
            attrs_table(page, &item.attrs);
        }
        for function in &module.functions {
            symbol_heading(page, &function.name, "Macro or function.");
            if function.params.is_empty() {
                page.push_str("No arguments.\n");
                continue;
            }
            page.push_str("| Argument | Required | Default |\n| --- | --- | --- |\n");
            for param in &function.params {
                row(
                    page,
                    &[
                        code(&param.name),
                        yes_no(param.mandatory),
                        optional_default(&param.default),
                    ],
                );
            }
        }
        for provider in &module.providers {
            symbol_heading(page, &provider.name, "Provider.");
            if provider.fields.is_empty() {
                page.push_str("No fields.\n");
            } else {
                page.push_str(&format!("Fields: {}.\n", list(&provider.fields, "")));
            }
        }
        for aspect in &module.aspects {
            symbol_heading(page, &aspect.name, "Aspect.");
            attrs_table(page, &aspect.attrs);
        }
        for rule in &module.repository_rules {
            symbol_heading(page, &rule.name, "Repository rule.");
            attrs_table(page, &rule.attrs);
        }
        for extension in &module.extensions {
            symbol_heading(page, &extension.name, "Module extension.");
            if extension.tag_classes.is_empty() {
                page.push_str("No tags.\n");
            }
            for (index, tag) in extension.tag_classes.iter().enumerate() {
                if index > 0 {
                    page.push('\n');
                }
                page.push_str(&format!("Tag `{}.{}`:\n\n", extension.name, tag.name));
                attrs_table(page, &tag.attrs);
            }
        }
    }
}

fn symbol_heading(page: &mut String, name: &str, kind: &str) {
    page.push_str(&format!("\n#### `{name}`\n\n{kind}\n\n"));
}

fn attrs_table(page: &mut String, attrs: &[Attr]) {
    if attrs.is_empty() {
        page.push_str("No attributes.\n");
        return;
    }
    page.push_str("| Attribute | Type | Required | Default |\n| --- | --- | --- | --- |\n");
    for attr in attrs {
        row(
            page,
            &[
                code(&attr.name),
                code(&attr.r#type.to_lowercase()),
                yes_no(attr.mandatory),
                optional_default(&attr.default),
            ],
        );
    }
}

fn optional_default(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        default_cell(text)
    }
}

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
    let mut hubs = Vec::new();
    let mut targets = Vec::new();
    let mut runnables = Vec::new();
    for row in &rows {
        match row.rule {
            "platform" => platforms.push(absolute(row.label)),
            "toolchain_type" => toolchain_types.push(absolute(row.label)),
            "hub" => hubs.push([code(row.label), code(&row.default)]),
            "runnable" => runnables.push([code(&absolute(row.label)), code(&row.default)]),
            _ if !row.label.starts_with("//config:") && !row.rule.ends_with("_flag") => {
                targets.push([code(&absolute(row.label)), code(row.rule)]);
            }
            _ => self::row(
                page,
                &[
                    code(&absolute(row.label)),
                    code(row.rule),
                    if row.default.is_empty() {
                        String::new()
                    } else {
                        default_cell(&setting_value(row.label, &row.default))
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
    page.push_str("\n## Hub repositories\n\n");
    page.push_str("| Repository | Extension |\n| --- | --- |\n");
    for cells in &hubs {
        row(page, cells);
    }
    page.push_str("\n## Targets\n\n");
    page.push_str("| Target | Kind |\n| --- | --- |\n");
    for cells in &targets {
        row(page, cells);
    }
    page.push_str("\n## Runnable targets\n\nRun these with `bazel run <target>`.\n\n");
    page.push_str("| Target | Kind |\n| --- | --- |\n");
    for cells in &runnables {
        row(page, cells);
    }
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

fn unquote(text: &str) -> Option<String> {
    if text.starts_with('"') {
        serde_json::from_str(text).ok()
    } else {
        Some(text.to_owned())
    }
}

fn absolute(label: &str) -> String {
    format!("{MODULE}{label}")
}

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

fn default_cell(value: &str) -> String {
    let private = value.match_indices("/private").any(|(at, word)| {
        value[at + word.len()..]
            .chars()
            .next()
            .is_none_or(|next| !(next.is_ascii_alphanumeric() || "_-.".contains(next)))
    });
    if private {
        "pinned".to_owned()
    } else {
        code(value)
    }
}

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
            "//config:config_tier string_flag \"strict\" [\"standard\", \"strict\"]\n",
            "//config:tool_dart label_flag \":no_dart\"\n",
            "//config:tool_ruff label_flag \"@dx_tools//:ruff\"\n",
            "//config:tool_hlint label_flag \"//haskell/private/tools/hlint/bin\"\n",
            "//config:toolchains bool_flag False\n",
            "//apple:macos_arm64 platform\n",
            "//cuda:gpu_driver string_flag \"auto\" [\"auto\", \"none\"]\n",
            "//cuda:is_sm_90 config_setting\n",
            "//lua:toolchain_type toolchain_type\n",
            "//:dx alias\n",
            "//deploy:bootstrap_offline runnable native_binary\n",
            "@demo_crates hub @rules_dx//rust:extensions.bzl%cargo\n",
        );
        let starlark = r#"[{"load":"@rules_dx//demo:defs.bzl",
            "rules":[{"name":"demo_test","executable":true,"test":true,"attrs":[
                {"name":"srcs","type":"LABEL_LIST","default":"[]","mandatory":false},
                {"name":"name","type":"NAME","default":"","mandatory":true}]}],
            "macros":[],"aspects":[],"repository_rules":[],
            "functions":[{"name":"demo_binary","params":[{"name":"name","default":"","mandatory":true},
                {"name":"mode","default":"\"a|b\"","mandatory":false},
                {"name":"tool","default":"Label(\"@rules_dx//demo/private:tool\")","mandatory":false}]}],
            "providers":[{"name":"DemoInfo","fields":["files","roots"]}],
            "extensions":[{"name":"demo","tag_classes":[{"name":"lock","attrs":[
                {"name":"lock","type":"LABEL","default":"","mandatory":true}]}]}]}]"#;
        let page = render(cli, starlark, labels, "events.schema.json 4.0 sha256:aa\n")
            .expect("page renders");
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
            "| `@rules_dx//config:config_tier` | `string_flag` | `strict` | `standard`, `strict` |",
            "| `@rules_dx//config:tool_dart` | `label_flag` | `@rules_dx//config:no_dart` |  |",
            "| `@rules_dx//config:tool_ruff` | `label_flag` | `@dx_tools//:ruff` |  |",
            "| `@rules_dx//config:tool_hlint` | `label_flag` | pinned |  |",
            "| `@rules_dx//config:toolchains` | `bool_flag` | `False` |  |",
            "- `@rules_dx//apple:macos_arm64`",
            "| `@rules_dx//cuda:gpu_driver` | `string_flag` | `auto` | `auto`, `none` |",
            "| `@rules_dx//cuda:is_sm_90` | `config_setting` |",
            "- `@rules_dx//lua:toolchain_type`",
            "| `events.schema.json` | `4.0` |",
            "| `@demo_crates` | `@rules_dx//rust:extensions.bzl%cargo` |",
            "| `@rules_dx//:dx` | `alias` |",
            "### `@rules_dx//demo:defs.bzl`",
            "#### `demo_test`",
            "Test rule. Run it with `bazel test`.",
            "| `srcs` | `label_list` | no | `[]` |",
            "| `name` | `name` | yes |  |",
            "#### `demo_binary`",
            "| `mode` | no | `\"a\\|b\"` |",
            "| `tool` | no | pinned |",
            "Fields: `files`, `roots`.",
            "Tag `demo.lock`:",
            "| `lock` | `label` | yes |  |",
        ] {
            assert!(
                page.lines().any(|got| got == line),
                "missing {line:?} in\n{page}"
            );
        }
        assert!(!page.contains("/private/"), "{page}");
        let settings = page
            .split_once("## Build settings")
            .and_then(|(_, rest)| rest.split_once("\n## "))
            .map(|(table, _)| table)
            .expect("a Build settings section");
        for label in ["@demo_crates", "//:dx", "bootstrap_offline", "is_sm_90"] {
            assert!(
                !settings.contains(label),
                "{label} in Build settings:\n{settings}"
            );
        }
        let runnables = page
            .split_once("## Runnable targets")
            .map(|(_, rest)| rest.split_once("\n## ").map_or(rest, |(table, _)| table))
            .expect("a Runnable targets section");
        assert!(
            runnables
                .lines()
                .any(|line| line == "| `@rules_dx//deploy:bootstrap_offline` | `native_binary` |"),
            "{runnables}"
        );
    }

    #[test]
    fn a_broken_snapshot_is_an_error() {
        assert!(render("{}", "[]", "", "").is_err());
        let cli = r#"{"commands":[],"environment":[]}"#;
        assert!(render(cli, "[{}]", "", "").is_err());
    }

    #[test]
    fn a_broken_labels_line_is_an_error() {
        let cli = r#"{"commands":[],"environment":[]}"#;
        assert!(render(cli, "[]", "//config:x\n", "").is_err());
        assert!(render(cli, "[]", "//config:x string_flag \"open\n", "").is_err());
        assert!(render(cli, "[]", "//config:x string_flag \"a\" not-a-list\n", "").is_err());
    }
}
