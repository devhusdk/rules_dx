use dx_testing::{read_runfiles, runfiles_root};

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
    let start = text
        .lines()
        .position(|line| line.trim().starts_with(prefix.as_str()))
        .unwrap_or_else(|| panic!("{key} is unset"));
    let mut items = Vec::new();
    for (index, line) in text.lines().skip(start).enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let line = if index == 0 {
            &line[prefix.len()..]
        } else {
            line
        };
        let (body, done) = match line.split_once(']') {
            Some((body, _)) => (body, true),
            None => (line, false),
        };
        items.extend(
            body.split(',')
                .map(|item| item.trim().trim_matches('"').to_owned())
                .filter(|item| !item.is_empty()),
        );
        if done {
            break;
        }
    }
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

/// Quoted ids in the `TIER_BASE_TOOLS` const of `staging.rs`.
fn runner_tier_base_tools(source: &str) -> Vec<String> {
    let marker = "const TIER_BASE_TOOLS: &[&str] = &[";
    let open = source.find(marker).expect("TIER_BASE_TOOLS is missing") + marker.len();
    let close = open + source[open..].find(']').expect("TIER_BASE_TOOLS is unterminated");
    json_strings(&source[open..close])
}

#[test]
fn the_matrix_tier_base_list_matches_the_runner_tier_base_tools() {
    let runner = runner_tier_base_tools(&read_runfiles("core/quality/runner/src/real/staging.rs"));
    assert!(!runner.is_empty(), "no tier base tools parsed");
    assert_eq!(
        runner,
        starlark_list_items(
            &read_runfiles("core/quality/testdata/runner_matrix_tests.bzl"),
            "RUNNER_TIER_BASE_TOOLS",
        ),
        "RUNNER_TIER_BASE_TOOLS in core/quality/testdata/runner_matrix_tests.bzl no longer \
         matches TIER_BASE_TOOLS in core/quality/runner/src/real/staging.rs"
    );
}

#[test]
fn the_tools_the_runner_refuses_without_a_config_are_the_tools_it_cannot_default() {
    let refusals = runner_config_refusals(&read_runfiles("core/quality/runner/real_sources.txt"));
    let effective = refusals;
    assert_eq!(
        effective,
        starlark_list_items(
            &read_runfiles("core/quality/native_config_tests.bzl"),
            "_RUNNER_CONFIG_REQUIRED_TOOLS",
        ),
        "the tools the runner refuses without a config no longer match the set \
         core/quality/native_config_tests.bzl pins"
    );
    let required = starlark_dict_keys(
        &read_runfiles("core/quality/private/native_config_impl.bzl"),
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

fn bazel_floor(module: &str) -> String {
    let list = module
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("bazel_compatibility = ["))
        .expect("bazel_compatibility is unset");
    list.split(']')
        .next()
        .expect("bazel_compatibility is unterminated")
        .trim()
        .trim_matches('"')
        .trim_start_matches(">=")
        .to_owned()
}

#[test]
fn bazel_floor_reads_the_compatibility_entry() {
    assert_eq!(
        bazel_floor("module(\n    bazel_compatibility = [\">=9.2.0\"],\n)"),
        "9.2.0"
    );
}

#[test]
fn bazel_compatibility_floor_matches_bazelversion() {
    assert_eq!(
        bazel_floor(&read_runfiles("MODULE.bazel")),
        read_runfiles(".bazelversion").trim(),
        "MODULE.bazel bazel_compatibility and .bazelversion disagree"
    );
}

#[test]
fn toml_list_reads_one_line_and_commented_lists() {
    assert_eq!(toml_list("select = [\"B\", \"A\"]\n", "select"), ["A", "B"]);
    assert_eq!(
        toml_list(
            "ignore = [\n  \"S101\", # tests assert\n  \"ARG001\",\n]\n",
            "ignore"
        ),
        ["ARG001", "S101"]
    );
}

#[test]
fn ruff_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles("ruff.toml");
    let fixture = read_runfiles("core/quality/testdata/ruff.toml");
    assert_eq!(
        toml_scalar(&workspace, "target-version"),
        toml_scalar(&fixture, "target-version"),
        "ruff.toml and core/quality/testdata/ruff.toml disagree on the Python floor"
    );
    assert_eq!(
        toml_list(&workspace, "ignore"),
        toml_list(&fixture, "ignore"),
        "ruff.toml and core/quality/testdata/ruff.toml disagree on the ignore list"
    );
}

#[test]
fn buildifier_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles(".buildifier.json");
    let fixture = read_runfiles("core/quality/testdata/buildifier_cfg/.buildifier.json");
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
    let corpus = read_runfiles("core/quality/corpus_vale.ini");
    let fixture = read_runfiles("core/quality/testdata/vale_test.ini");
    for key in ["MinAlertLevel", "BasedOnStyles"] {
        assert_eq!(
            ini_value(&shim, key),
            ini_value(&corpus, key),
            ".vale.ini and core/quality/corpus_vale.ini disagree on {key}"
        );
        assert_eq!(
            ini_value(&shim, key),
            ini_value(&fixture, key),
            ".vale.ini and core/quality/testdata/vale_test.ini disagree on {key}"
        );
    }
}

#[test]
fn every_vale_config_resolves_its_marker_style() {
    for (config, declared, styles) in [
        (
            ".vale.ini",
            "core/quality/corpus_styles",
            "core/quality/corpus_styles",
        ),
        (
            "core/quality/corpus_vale.ini",
            "corpus_styles",
            "core/quality/corpus_styles",
        ),
        (
            "core/quality/testdata/vale_test.ini",
            "styles",
            "core/quality/testdata/styles",
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
    let fixture = read_runfiles("core/quality/testdata/styles/Dx/Markers.yml");
    let corpus = read_runfiles("core/quality/corpus_styles/Dx/Markers.yml");
    assert_eq!(
        yaml_body(&fixture),
        yaml_body(&corpus),
        "the hinted Dx.Markers style drifted from core/quality/corpus_styles"
    );
}

#[test]
fn biome_policy_matches_the_hinted_fixture() {
    let workspace = read_runfiles("biome.json");
    let fixture = read_runfiles("core/quality/testdata/biome_cfg/biome.json");
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
            "{config} names shfmt; core/quality/adapter/src/commands.rs owns that invocation"
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

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Returns the name of the dx state directory, read from its constant in dx_path.
fn dx_dir_name() -> String {
    const SOURCE: &str = "cli/path/src/state.rs";
    const PREFIX: &str = "pub const DX_DIR: &str = \"";
    read_runfiles(SOURCE)
        .lines()
        .find_map(|line| line.trim().strip_prefix(PREFIX)?.strip_suffix("\";"))
        .unwrap_or_else(|| panic!("{SOURCE} declares no DX_DIR string constant"))
        .to_owned()
}

/// Returns true when an ignore file ignores `dir/` itself or one of its parents.
fn ignored_dir(text: &str, dir: &str) -> bool {
    let wanted = format!("{dir}/");
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(['#', '!']))
        .map(|line| line.trim_start_matches("**/").trim_start_matches('/'))
        .map(|line| line.trim_end_matches('*'))
        .filter(|line| line.ends_with('/'))
        .any(|line| wanted.starts_with(line))
}

/// Returns the dot directories `.bazelignore` lists, without a trailing slash.
fn bazelignored_dirs(text: &str) -> Vec<String> {
    let mut dirs: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('.') && !line.starts_with(".."))
        .map(|line| line.trim_end_matches('/').to_owned())
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

#[test]
fn dx_state_dir_is_ignored_by_git_bazel_and_editors() {
    let dir = dx_dir_name();
    let gitignore = read_runfiles(".gitignore");
    let bazelignore = read_runfiles(".bazelignore");
    let biome = read_runfiles("biome.json");
    let local = format!("{dir}/local");
    assert!(
        ignored_dir(&gitignore, &local),
        ".gitignore never ignores {local}/"
    );
    // rules_dx commits its generated .dx/bazelrc, so only Bazel skips it.
    for state in ["local", "bazelrc"] {
        let state = format!("{dir}/{state}");
        assert!(
            bazelignore
                .lines()
                .map(str::trim)
                .any(|line| line == state || line.starts_with(&format!("{state}/"))),
            ".bazelignore never ignores {state}/"
        );
    }
    assert!(
        biome.contains(&format!("\"!!**/{dir}\"")),
        "biome.json files.includes never force-ignores {dir}"
    );
}

#[test]
fn every_dot_dir_bazel_ignores_is_ignored_by_git_and_editors() {
    let bazelignore = read_runfiles(".bazelignore");
    let gitignore = read_runfiles(".gitignore");
    let biome = read_runfiles("biome.json");
    let dirs = bazelignored_dirs(&bazelignore);
    assert!(!dirs.is_empty(), ".bazelignore lists no dot dirs");
    let committed = format!("{}/bazelrc", dx_dir_name());
    for dir in dirs {
        assert!(
            dir == committed || ignored_dir(&gitignore, &dir),
            ".gitignore never ignores {dir}/ listed in .bazelignore"
        );
        let top = dir.split('/').next().unwrap_or(&dir);
        assert!(
            biome.contains(&format!("\"!!**/{top}\"")),
            "biome.json files.includes never force-ignores {dir} listed in .bazelignore"
        );
    }
}

/// Returns every workflow with its string values joined, so the scans skip comments.
fn workflow_values() -> Vec<(String, String)> {
    let found: Vec<(String, String)> = dx_testing::workflow_files()
        .into_iter()
        .map(|name| {
            let text = ci_workflow::strings(&name).join("\n");
            (name, text)
        })
        .collect();
    assert!(!found.is_empty(), "DX_WORKFLOWS names no workflow");
    found
}

fn label_chars(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | ':' | '_' | '.' | '-')
}

/// The three preset layers as the generator renders them for rules_dx, in import order.
fn preset_layers() -> Vec<(String, String)> {
    dx_preset::Layer::ALL
        .iter()
        .map(|layer| {
            let strict = (*layer == dx_preset::Layer::Correctness).then_some(true);
            (
                layer.file_name().to_owned(),
                dx_preset::render_layer_with(*layer, "rules_dx", strict),
            )
        })
        .collect()
}

fn preset_text() -> String {
    preset_layers().into_iter().map(|(_, text)| text).collect()
}

fn preset_layer(layer: dx_preset::Layer) -> String {
    dx_preset::render_layer_with(layer, "rules_dx", None)
}

/// Returns every `--min-coverage` value on a line that names coverage.
fn coverage_floors(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in text.lines() {
        if !line.contains("coverage") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("--min-coverage") {
            let tail = rest[start + "--min-coverage".len()..].trim_start_matches([' ', '=']);
            let end = tail.find(|ch: char| !label_chars(ch)).unwrap_or(tail.len());
            if end > 0 {
                found.push(tail[..end].to_owned());
            }
            rest = &tail[end..];
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Returns the coverage floor ci.yml passes to the required check.
fn required_coverage_floor() -> String {
    const CI: &str = ".github/workflows/ci.yml";
    let floors: Vec<String> = ci_workflow::jobs(CI)
        .into_iter()
        .filter_map(|(_, job)| job["with"]["min_coverage"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        floors.len(),
        1,
        "{CI} passes one coverage floor to the required check: {floors:?}"
    );
    floors[0].clone()
}

#[test]
fn every_workflow_coverage_floor_matches_the_required_check() {
    let expected = required_coverage_floor();
    let mut floors = 0;
    for (workflow, text) in workflow_values() {
        for floor in coverage_floors(&text) {
            assert_eq!(
                floor, expected,
                "{workflow} enforces a coverage floor the required check does not"
            );
            floors += 1;
        }
    }
    assert!(floors > 0, "no workflow runs dx coverage with a floor");
}

#[test]
fn required_coverage_floor_is_exact_100() {
    assert_eq!(
        required_coverage_floor(),
        "100",
        "ci.yml must hold the v1 exact-coverage floor at 100"
    );
}

#[test]
fn release_dry_run_enforces_the_required_coverage_floor() {
    let expected = required_coverage_floor();
    let release: Vec<String> = workflow_values()
        .into_iter()
        .filter(|(name, _)| {
            name.ends_with("workflows/release.yml") || name.ends_with("workflows/release-cells.yml")
        })
        .map(|(_, text)| text)
        .collect();
    assert_eq!(
        release.len(),
        2,
        "release.yml and release-cells.yml ship as config_consistency_test data"
    );
    assert!(
        release
            .iter()
            .flat_map(|text| coverage_floors(text))
            .any(|floor| floor == expected),
        "release.yml must qualify the release with dx coverage at {expected}"
    );
}

/// Returns every `--config=` name `text` passes, sorted and deduplicated.
fn workflow_bazel_configs(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("--config=") {
        let tail = &rest[start + "--config=".len()..];
        let end = tail.find(|ch: char| !label_chars(ch)).unwrap_or(tail.len());
        if end > 0 {
            found.push(tail[..end].to_owned());
        }
        rest = &tail[end..];
    }
    found.sort();
    found.dedup();
    found
}

/// Returns every config name a bazelrc defines with a `command:name` line.
fn bazelrc_configs(rc: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in rc.lines() {
        let trimmed = line.trim();
        let Some((command, rest)) = trimmed.split_once(':') else {
            continue;
        };
        if command.is_empty() || !command.chars().all(|ch| ch.is_ascii_lowercase()) {
            continue;
        }
        let name = rest.split(' ').next().unwrap_or_default();
        if !name.is_empty() {
            found.push(name.to_owned());
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Returns whether a config needs a docs stanza: preset configs do not, `.bazelrc` ones do.
fn config_needs_doc_stanza(config: &str, rc: &str, preset: &str) -> Result<bool, String> {
    if bazelrc_configs(preset).iter().any(|name| name == config) {
        return Ok(false);
    }
    if bazelrc_configs(rc).iter().any(|name| name == config) {
        return Ok(true);
    }
    Err(format!(
        "no .bazelrc or .dx/bazelrc preset layer line defines --config={config}"
    ))
}

#[test]
fn preset_configs_need_no_doc_stanza() {
    let rc = "common:ci --remote_timeout=10m\nbuild:dx_build_release --host_compilation_mode=opt\n";
    let preset = "build:dx_build_release --compilation_mode=opt\n";
    assert_eq!(
        config_needs_doc_stanza("dx_build_release", rc, preset),
        Ok(false)
    );
    assert_eq!(config_needs_doc_stanza("ci", rc, preset), Ok(true));
    assert!(config_needs_doc_stanza("missing", rc, preset).is_err());
}

/// Configs the starter fixtures define in the generated workspace's own `.bazelrc`.
const STARTER_CONFIGS: &[&str] = &["android"];

/// Configs only rules_dx's own evidence workflow passes, defined in the root `.bazelrc`.
const REPO_ONLY_CONFIGS: &[&str] = &["rbe"];

#[test]
fn every_workflow_bazel_config_is_defined_and_documented() {
    let rc = read_runfiles(".bazelrc");
    let preset = preset_text();
    let page = read_runfiles("docs/github-ci.md");
    let mut passes = 0;
    for (workflow, text) in workflow_values() {
        for config in workflow_bazel_configs(&text) {
            if STARTER_CONFIGS.contains(&config.as_str()) {
                continue;
            }
            let needs_stanza = config_needs_doc_stanza(&config, &rc, &preset)
                .unwrap_or_else(|err| panic!("{workflow} runs --config={config}; {err}"));
            assert!(
                !needs_stanza
                    || REPO_ONLY_CONFIGS.contains(&config.as_str())
                    || page.contains(&format!("common:{config} ")),
                "docs/github-ci.md never shows the common:{config} stanza --config={config} \
                 needs; only preset configs are exempt"
            );
            passes += 1;
        }
    }
    assert!(passes > 0, "no workflow passes a Bazel config");
}

/// Every `--override_repository` in `.bazelrc` that points into the workspace, as a workspace path.
fn workspace_overrides(rc: &str) -> Vec<(String, String)> {
    rc.lines()
        .filter(|line| !is_comment(line))
        .filter_map(|line| {
            line.split_once("--override_repository=")
                .map(|(_, value)| value)
        })
        .filter_map(|value| value.split_once("=%workspace%/"))
        .map(|(repo, dir)| (repo.to_string(), dir.trim().to_string()))
        .collect()
}

#[test]
fn windows_overrides_point_at_tracked_dirs() {
    let overrides = workspace_overrides(&read_runfiles(".bazelrc"));
    assert!(
        overrides
            .iter()
            .any(|(repo, _)| repo == "bazel_tools+winsdk_configure+local_config_winsdk"),
        ".bazelrc no longer replaces local_config_winsdk, so Windows jobs run vcvars again"
    );
    for (repo, dir) in overrides {
        read_runfiles(&format!("{dir}/BUILD.bazel"));
        // Without a repo boundary file Bazel refuses the directory as a repository.
        read_runfiles(&format!("{dir}/REPO.bazel"));
        if repo.ends_with("local_config_winsdk") {
            // bazel_tools loads register_local_rc_exe_toolchains from the replacement.
            let bzl = read_runfiles(&format!("{dir}/toolchains.bzl"));
            assert!(
                bzl.contains("def register_local_rc_exe_toolchains("),
                "{dir}/toolchains.bzl must define register_local_rc_exe_toolchains"
            );
        }
    }
}

#[test]
fn ci_config_leaves_remote_downloads_to_dx() {
    for path in [".bazelrc", "docs/github-ci.md"] {
        let text = read_runfiles(path);
        assert!(
            !text.contains("--remote_download_outputs=all"),
            "{path} sets --remote_download_outputs=all"
        );
    }
    // dx refuses --remote_download_minimal, so only evidence.yml passes it, on bazel itself.
    assert!(
        !read_runfiles(".bazelrc").contains("remote_download_minimal"),
        ".bazelrc sets --remote_download_minimal, which dx refuses"
    );
}

#[test]
// The preset turns the cache on, so local runs and consumers get it without a CI echo.
fn remote_repo_contents_cache_comes_from_the_preset() {
    let flag = "startup --experimental_remote_repo_contents_cache";
    let performance = preset_layer(dx_preset::Layer::Performance);
    assert!(performance.contains(flag), "the preset no longer sets {flag}");
    for path in [
        ".github/workflows/evidence.yml",
        ".github/workflows/reusable-consumer.yml",
    ] {
        assert!(!read_runfiles(path).contains(flag), "{path} echoes {flag} again");
    }
    assert!(
        !read_runfiles(".github/workflows/evidence.yml").contains("build --jobs=64"),
        "evidence.yml passes build --jobs=64 again"
    );
    assert!(
        !read_runfiles(".bazelrc").contains("loading_phase_threads"),
        ".bazelrc repeats a flag the preset owns"
    );
    assert!(
        performance.contains("common:ci --loading_phase_threads=32"),
        "the preset must let CI look up more repos at once"
    );
    assert!(
        performance.contains("common:ci --jobs=64"),
        "the preset no longer sets common:ci --jobs=64"
    );
}

const PLAIN_GIT_CHECKOUT: &str = "-c checkout.workers=0 checkout --force FETCH_HEAD";

fn step_runs(step: &serde_json::Value) -> &str {
    step.get("run").and_then(serde_json::Value::as_str).unwrap_or("")
}

fn uses_actions_checkout(step: &serde_json::Value) -> bool {
    step.get("uses")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|uses| uses.starts_with("actions/checkout@"))
}

#[test]
fn evidence_checks_out_in_parallel_without_actions_checkout() {
    const EVIDENCE: &str = ".github/workflows/evidence.yml";
    let mut parallel = 0;
    for (id, job) in ci_workflow::jobs_of(&ci_workflow::document(EVIDENCE)) {
        for step in ci_workflow::steps_of(job) {
            assert!(!uses_actions_checkout(step), "{EVIDENCE} {id} checks out with git, not actions/checkout");
            if step.get("name").and_then(serde_json::Value::as_str) == Some("Check out files in parallel") {
                parallel += 1;
                assert!(step.get("if").is_none(), "the parallel checkout must run on every OS");
                assert!(
                    step_runs(step).contains(PLAIN_GIT_CHECKOUT),
                    "the checkout must run with checkout.workers=0"
                );
            }
        }
    }
    assert!(parallel > 0, "{EVIDENCE} has no parallel checkout step");
}

#[test]
fn ci_windows_jobs_check_out_with_git() {
    const CI: &str = ".github/workflows/ci.yml";
    for name in ["hermetic-windows", "starter-consumer", "preflight"] {
        let steps = ci_workflow::steps(CI, name);
        assert!(
            steps.iter().any(|step| step_runs(step).contains(PLAIN_GIT_CHECKOUT)),
            "ci.yml {name} must check out with git and checkout.workers=0"
        );
        assert!(
            !steps.iter().any(uses_actions_checkout),
            "ci.yml {name} checks out with git, not actions/checkout"
        );
    }
}

#[test]
fn windows_capable_jobs_check_out_in_parallel() {
    for path in [
        ".github/workflows/evidence.yml",
        ".github/workflows/ci.yml",
        ".github/workflows/reusable-consumer.yml",
    ] {
        for (id, job) in ci_workflow::jobs_of(&ci_workflow::document(path)) {
            let runner = job.get("runs-on").map_or_else(String::new, ToString::to_string);
            let runner = runner.trim_matches('"');
            let windows_capable = !(runner.starts_with("ubuntu-") || runner.starts_with("macos-"));
            let steps = ci_workflow::steps_of(job);
            if !windows_capable || steps.is_empty() {
                continue;
            }
            assert!(
                steps.iter().any(|step| step_runs(step).contains(PLAIN_GIT_CHECKOUT)),
                "{path} {id} can run on Windows but does not check out with plain git and -c checkout.workers=0"
            );
            assert!(
                !steps.iter().any(uses_actions_checkout),
                "{path} {id} can run on Windows but uses actions/checkout"
            );
        }
    }
}

#[test]
fn the_scalafmt_tiers_pin_the_version_the_toolchain_downloads() {
    let version_of = |text: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix("version = "))
            .expect("version is unset")
            .trim()
            .to_owned()
    };
    let artifact = read_runfiles("core/quality/artifacts/scalafmt.bzl");
    for tier in ["standard", "strict"] {
        let path = format!("core/quality/tiers/scalafmt/{tier}.conf");
        let pinned = version_of(&read_runfiles(&path));
        assert_eq!(pinned, version_of(&read_runfiles(".scalafmt.conf")), "{path}");
        assert!(
            artifact.contains(&format!("\"upstream_version\": \"{pinned}\"")),
            "{path} pins {pinned}, which core/quality/artifacts/scalafmt.bzl does not ship"
        );
    }
}

#[test]
fn host_dependent_extensions_are_locked_for_every_ci_platform() {
    let lock: serde_json::Value = serde_json::from_str(&read_runfiles("MODULE.bazel.lock"))
        .expect("MODULE.bazel.lock is JSON");
    let platforms = [
        "os:linux,arch:amd64",
        "os:linux,arch:aarch64",
        "os:osx,arch:aarch64",
        "os:windows,arch:amd64",
        "os:windows,arch:aarch64",
    ];
    for extension in [
        "@@rules_python+//python/extensions:python.bzl%python",
        "@@rules_go+//go:extensions.bzl%go_sdk",
    ] {
        let entries = &lock["moduleExtensions"][extension];
        let missing: Vec<&str> = platforms
            .into_iter()
            .filter(|platform| entries.get(platform).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "MODULE.bazel.lock has no {extension} entry for {missing:?}; dispatch lock-refresh.yml and commit its MODULE.bazel.lock artifact"
        );
    }
}

#[test]
fn the_network_job_selects_a_tag_the_network_tests_carry() {
    let bzl = read_runfiles("tools/testing/nested_bazel.bzl");
    let line = bzl
        .lines()
        .find(|line| line.starts_with("NESTED_BAZEL_NETWORK_TAGS ="))
        .expect("NESTED_BAZEL_NETWORK_TAGS is missing");
    let queried: Vec<String> = workflow_values()
        .into_iter()
        .filter(|(name, _)| name.ends_with("workflows/ci.yml"))
        .flat_map(|(_, text)| {
            text.split("attr(tags, ")
                .skip(1)
                .filter_map(|rest| rest.split(',').next().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        queried.iter().any(|tag| tag == "network-consumer"),
        "ci.yml no longer queries the network-consumer tag"
    );
    assert!(
        line.contains("\"network-consumer\""),
        "ci.yml selects network-consumer but NESTED_BAZEL_NETWORK_TAGS does not carry it"
    );
}
