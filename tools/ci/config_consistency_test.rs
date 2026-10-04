use dx_testing::{read_runfiles, runfiles_root};

const GITHUB_BUILD: &str = ".github/BUILD.bazel";

fn squeezed(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn toml_scalar(text: &str, key: &str) -> String {
    let prefix = format!("{key} = ");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{key} is unset"))
        .trim()
        .trim_matches('"')
        .to_owned()
}

fn toml_list(text: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key} = [");
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(prefix.as_str()))
        .unwrap_or_else(|| panic!("{key} is unset"));
    let mut items: Vec<String> = line[prefix.len()..]
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    items.sort();
    items
}

fn ini_value(text: &str, key: &str) -> String {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(key))
        .unwrap_or_else(|| panic!("{key} is unset"))
        .trim_start_matches([' ', '='])
        .trim()
        .to_owned()
}

fn json_object(text: &str, key: &str) -> String {
    let marker = format!("\"{key}\"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{key} is missing"));
    let open = start
        + text[start..]
            .find('{')
            .unwrap_or_else(|| panic!("{key} is no object"));
    let mut depth = 0_usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return text[open..open + offset + 1].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("{key} is unterminated");
}

fn json_array(text: &str, key: &str) -> String {
    let marker = format!("\"{key}\":");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{key} is missing"));
    let open = start
        + text[start..]
            .find('[')
            .unwrap_or_else(|| panic!("{key} is no array"));
    let close = open
        + text[open..]
            .find(']')
            .unwrap_or_else(|| panic!("{key} is unterminated"));
    text[open..close + 1].to_owned()
}

fn json_strings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        let end = rest
            .find('"')
            .unwrap_or_else(|| panic!("{text} has a dangling quote"));
        found.push(rest[..end].to_owned());
        rest = &rest[end + 1..];
    }
    found
}

fn yaml_body(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn starlark_dict_keys(text: &str, name: &str) -> Vec<String> {
    let open = text
        .find(&format!("{name} = {{"))
        .unwrap_or_else(|| panic!("{name} is missing"))
        + name.len()
        + 4;
    let close = open + text[open..].find('}').expect("dict is unterminated");
    json_strings(&text[open..close])
}

fn starlark_list_items(text: &str, name: &str) -> Vec<String> {
    let marker = format!("{name} = [");
    let open = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} is missing"))
        + marker.len();
    let close = open + text[open..].find(']').expect("list is unterminated");
    json_strings(&text[open..close])
}

/// Tool ids the runner names in a `"<tool> requires a config"` error.
fn runner_config_refusals(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(" requires a config\"") {
        let head = &rest[..at];
        let open = head.rfind('"').expect("refusal literal opens with a quote");
        found.push(head[open + 1..].to_owned());
        rest = &rest[at + 1..];
    }
    found.sort();
    found.dedup();
    found
}

/// Tool ids the runner materializes a default config for.
fn defaulted_config_tools(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    let marker = "if tool_id == \"";
    while let Some(at) = rest.find(marker) {
        let after = at + marker.len();
        let close = after + rest[after..].find('"').expect("tool id closes");
        if rest[close..].starts_with("\" && tool.config_rel.is_none()") {
            found.push(rest[after..close].to_owned());
        }
        rest = &rest[close..];
    }
    found.sort();
    found
}

#[test]
fn the_tools_the_runner_refuses_without_a_config_are_the_tools_it_cannot_default() {
    let refusals = runner_config_refusals(&read_runfiles("quality/runner/src/real/check.rs"));
    assert!(
        !refusals.is_empty(),
        "the runner refuses no tool without a config"
    );
    let defaulted = defaulted_config_tools(&read_runfiles("quality/runner/src/real/staging.rs"));
    let effective: Vec<String> = refusals
        .iter()
        .filter(|tool| !defaulted.contains(tool))
        .cloned()
        .collect();
    assert_eq!(
        effective,
        starlark_list_items(
            &read_runfiles("quality/native_config_tests.bzl"),
            "_RUNNER_CONFIG_REQUIRED_TOOLS",
        ),
        "the tools the runner refuses without a config no longer match the set \
         quality/native_config_tests.bzl pins"
    );
    let required = starlark_dict_keys(
        &read_runfiles("quality/native_config.bzl"),
        "CONFIG_REQUIRED_TOOLS",
    );
    for tool in &effective {
        assert!(
            required.contains(tool),
            "the runner refuses {tool} without a config, but CONFIG_REQUIRED_TOOLS lets the \
             aspect build the action anyway, so the failure lands on the action instead of analysis"
        );
    }
}

#[test]
fn ruff_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles("ruff.toml");
    let fixture = read_runfiles("quality/testdata/ruff.toml");
    let selected = toml_list(&workspace, "select");
    assert!(!selected.is_empty(), "ruff.toml selects no lint");
    assert_eq!(
        toml_scalar(&workspace, "target-version"),
        toml_scalar(&fixture, "target-version"),
        "ruff.toml and quality/testdata/ruff.toml disagree on the Python floor"
    );
    assert_eq!(
        selected,
        toml_list(&fixture, "select"),
        "ruff.toml and quality/testdata/ruff.toml disagree on the lint selection"
    );
}

#[test]
fn buildifier_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles(".buildifier.json");
    let fixture = read_runfiles("quality/testdata/buildifier_cfg/.buildifier.json");
    let selected = json_strings(&json_array(&workspace, "warningsList"));
    assert!(!selected.is_empty(), ".buildifier.json enables no warning");
    assert_eq!(
        selected,
        json_strings(&json_array(&fixture, "warningsList")),
        ".buildifier.json and the hinted fixture disagree on warningsList"
    );
}

#[test]
fn vale_policy_agrees_across_the_root_shim_the_corpus_and_the_fixture() {
    let shim = read_runfiles(".vale.ini");
    let corpus = read_runfiles("quality/corpus_vale.ini");
    let fixture = read_runfiles("quality/testdata/vale_test.ini");
    for key in ["MinAlertLevel", "BasedOnStyles"] {
        assert_eq!(
            ini_value(&shim, key),
            ini_value(&corpus, key),
            ".vale.ini and quality/corpus_vale.ini disagree on {key}"
        );
        assert_eq!(
            ini_value(&shim, key),
            ini_value(&fixture, key),
            ".vale.ini and quality/testdata/vale_test.ini disagree on {key}"
        );
    }
}

#[test]
fn every_vale_config_resolves_its_marker_style() {
    for (config, declared, styles) in [
        (
            ".vale.ini",
            "quality/corpus_styles",
            "quality/corpus_styles",
        ),
        (
            "quality/corpus_vale.ini",
            "corpus_styles",
            "quality/corpus_styles",
        ),
        (
            "quality/testdata/vale_test.ini",
            "styles",
            "quality/testdata/styles",
        ),
    ] {
        assert_eq!(
            ini_value(&read_runfiles(config), "StylesPath"),
            declared,
            "{config} declares a StylesPath this workspace does not ship"
        );
        let style = runfiles_root().join(styles).join("Dx/Markers.yml");
        assert!(
            style.is_file(),
            "{config} resolves StylesPath to {styles}, which ships no Dx/Markers.yml"
        );
    }
}

#[test]
fn the_hinted_marker_style_matches_the_corpus() {
    let fixture = read_runfiles("quality/testdata/styles/Dx/Markers.yml");
    let corpus = read_runfiles("quality/corpus_styles/Dx/Markers.yml");
    assert_eq!(
        yaml_body(&fixture),
        yaml_body(&corpus),
        "the hinted Dx.Markers style drifted from quality/corpus_styles"
    );
}

#[test]
fn biome_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles("biome.json");
    let fixture = read_runfiles("quality/testdata/biome_cfg/biome.json");
    for key in ["linter", "formatter"] {
        assert_eq!(
            squeezed(&json_object(&workspace, key)),
            squeezed(&json_object(&fixture, key)),
            "biome.json and the hinted fixture disagree on {key}"
        );
    }
    let scoped = json_strings(&json_array(&json_object(&workspace, "files"), "includes"));
    for entry in json_strings(&json_array(&json_object(&fixture, "files"), "includes")) {
        assert!(
            scoped.contains(&entry),
            "biome.json files.includes no longer lists {entry}, which the hinted fixture runs"
        );
    }
}

const SHELL_CONFIGS: [&str; 2] = [".editorconfig", ".shellcheckrc"];

/// The C/C++ sources clang-format owns. Test fixtures are excluded on purpose:
/// they carry third-party code and deliberately unsorted samples.
const CPP_SOURCES: [&str; 5] = [
    "examples/adopt-cpp/greet/greet.cc",
    "examples/adopt-cpp/greet/greet.h",
    "examples/adopt-cpp/greet/helper.cc",
    "examples/adopt-cpp/solo/pure.cc",
    "examples/adopt-polyglot/native_cpp/shapes.cc",
];

/// Comment words that name a file inside the workspace.
fn named_files(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('#'))
        .flat_map(|line| line.split_whitespace())
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && !".-_".contains(c)))
        .filter(|token| {
            token.contains('/')
                && token
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name.contains('.'))
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_shell_configs_name_only_files_the_workspace_ships() {
    let root = runfiles_root();
    for config in SHELL_CONFIGS {
        for name in named_files(&read_runfiles(config)) {
            assert!(
                root.join(&name).exists(),
                "{config} names {name}, which this workspace does not ship"
            );
        }
    }
}

#[test]
fn the_shell_configs_never_restate_the_shfmt_invocation() {
    for config in SHELL_CONFIGS {
        assert!(
            !read_runfiles(config).contains("shfmt"),
            "{config} names shfmt; quality/adapter/src/commands.rs owns that invocation"
        );
    }
}

/// Every code named on the shellcheckrc disable line.
fn disabled_shellcheck_codes(text: &str) -> Vec<&str> {
    let list = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("disable="))
        .expect("disable= is unset");
    list.split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .collect()
}

#[test]
fn every_disabled_shellcheck_code_has_a_rationale() {
    let text = read_runfiles(".shellcheckrc");
    let codes = disabled_shellcheck_codes(&text);
    assert!(!codes.is_empty(), ".shellcheckrc disables no code");
    let comments: Vec<&str> = text.lines().filter(|line| is_comment(line)).collect();
    for code in codes {
        assert!(
            comments.iter().any(|comment| comment.contains(code)),
            ".shellcheckrc disables {code} and no comment names it"
        );
    }
}

#[test]
fn every_shellcheck_rationale_names_a_disabled_code() {
    let text = read_runfiles(".shellcheckrc");
    let codes = disabled_shellcheck_codes(&text);
    for (index, line) in text.lines().enumerate() {
        if !is_comment(line) {
            continue;
        }
        assert!(
            codes.iter().any(|code| line.contains(code)),
            ".shellcheckrc:{}: the comment names no disabled code, so it wraps another: {line:?}",
            index + 1
        );
    }
}

/// Words AGENTS.md bans from a comment: issue references, design documents, and pointers at another page.
const BANNED_COMMENT_WORDS: [&str; 6] = ["issue", "ADR", "RFC", "Contract", "See:", "dedup"];

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Asserts every comment in `text` stays free of the banned vocabulary.
fn assert_comments_bare(name: &str, text: &str) {
    for (index, line) in text.lines().enumerate() {
        if !is_comment(line) {
            continue;
        }
        let body = line.trim_start().trim_start_matches('#').trim();
        for word in BANNED_COMMENT_WORDS {
            assert!(
                !body.contains(word),
                "{name}:{}: the comment says {word:?}, which AGENTS.md bans: {body:?}",
                index + 1
            );
        }
        assert!(
            !body.contains(".md"),
            "{name}:{}: the comment points at a page instead of the code: {body:?}",
            index + 1
        );
    }
}

/// The line number of every comment that directly follows another comment.
fn stacked_comments(name: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut previous = false;
    for (index, line) in text.lines().enumerate() {
        let here = is_comment(line);
        if here && previous {
            out.push(format!("{name}:{}", index + 1));
        }
        previous = here;
    }
    out
}

fn workflow_exports() -> Vec<String> {
    dx_testing::workflow_exports(&read_runfiles(GITHUB_BUILD), GITHUB_BUILD)
}

#[test]
fn every_workflow_comment_is_one_line() {
    let mut comments = 0usize;
    for name in workflow_exports() {
        let rel = format!(".github/{name}");
        let text = read_runfiles(&rel);
        let stacked = stacked_comments(&rel, &text);
        assert!(
            stacked.is_empty(),
            "{} stacks a comment on the line above; AGENTS.md allows one line: {}",
            rel,
            stacked.join(", ")
        );
        comments += text.lines().filter(|line| is_comment(line)).count();
    }
    assert!(comments > 0, "the workflow sweep found no comment to check");
}

#[test]
fn no_workflow_comment_names_an_issue_or_another_page() {
    for name in workflow_exports() {
        let rel = format!(".github/{name}");
        assert_comments_bare(&rel, &read_runfiles(&rel));
    }
}

#[test]
fn the_emitted_runfiles_block_names_no_issue_or_another_page() {
    assert_comments_bare(
        "deploy/rules/launcher.bzl",
        &read_runfiles("deploy/rules/launcher.bzl"),
    );
}

#[test]
fn the_clang_format_config_ships() {
    let text = read_runfiles(".clang-format");
    assert!(
        text.contains("BasedOnStyle: LLVM"),
        ".clang-format must pin a base style so a clang-format upgrade cannot restyle the tree"
    );
}

#[test]
fn every_owned_cpp_source_shipped_by_the_filegroups_exists() {
    let root = runfiles_root();
    for source in CPP_SOURCES {
        assert!(
            root.join(source).exists(),
            "CPP_SOURCES names {source}, which this workspace does not ship"
        );
    }
}
