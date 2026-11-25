use dx_testing::{read_runfiles, resolve_runfiles, runfiles_root};
use starlark_syntax as _;
use yaml_serde as _;

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
            line.get(prefix.len()..)
                .expect("string slice within bounds")
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
        + text
            .get(start..)
            .expect("string slice within bounds")
            .find('{')
            .unwrap_or_else(|| panic!("{key} is no object"));
    let mut depth = 0_usize;
    for (offset, ch) in text
        .get(open..)
        .expect("string slice within bounds")
        .char_indices()
    {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return text
                        .get(open..open + offset + 1)
                        .expect("string slice within bounds")
                        .to_owned();
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
        + text
            .get(start..)
            .expect("string slice within bounds")
            .find('[')
            .unwrap_or_else(|| panic!("{key} is no array"));
    let close = open
        + text
            .get(open..)
            .expect("string slice within bounds")
            .find(']')
            .unwrap_or_else(|| panic!("{key} is unterminated"));
    text.get(open..close + 1)
        .expect("string slice within bounds")
        .to_owned()
}

fn json_strings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        rest = rest.get(start + 1..).expect("string slice within bounds");
        let end = rest
            .find('"')
            .unwrap_or_else(|| panic!("{text} has a dangling quote"));
        found.push(
            rest.get(..end)
                .expect("string slice within bounds")
                .to_owned(),
        );
        rest = rest.get(end + 1..).expect("string slice within bounds");
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
    let close = open
        + text
            .get(open..)
            .expect("string slice within bounds")
            .find('}')
            .expect("dict is unterminated");
    json_strings(text.get(open..close).expect("string slice within bounds"))
}

fn starlark_list_items(text: &str, name: &str) -> Vec<String> {
    let marker = format!("{name} = [");
    let open = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} is missing"))
        + marker.len();
    let close = open
        + text
            .get(open..)
            .expect("string slice within bounds")
            .find(']')
            .expect("list is unterminated");
    json_strings(text.get(open..close).expect("string slice within bounds"))
}

fn runner_config_refusals(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(" requires a config\"") {
        let head = rest.get(..at).expect("string slice within bounds");
        let open = head.rfind('"').expect("refusal literal opens with a quote");
        found.push(
            head.get(open + 1..)
                .expect("string slice within bounds")
                .to_owned(),
        );
        rest = rest.get(at + 1..).expect("string slice within bounds");
    }
    found.sort();
    found.dedup();
    found
}

fn runner_tier_base_tools(source: &str) -> Vec<String> {
    let marker = "const TIER_BASE_TOOLS: &[&str] = &[";
    let open = source.find(marker).expect("TIER_BASE_TOOLS is missing") + marker.len();
    let close = open
        + source
            .get(open..)
            .expect("string slice within bounds")
            .find(']')
            .expect("TIER_BASE_TOOLS is unterminated");
    json_strings(source.get(open..close).expect("string slice within bounds"))
}

fn bound_tier_base_tools(generated: &str) -> Vec<String> {
    let mut id = "";
    let mut found = Vec::new();
    for line in generated.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("id: \"") {
            id = value.trim_end_matches("\",");
        } else if line == "tier_base: true," {
            found.push(id.to_owned());
        }
    }
    found
}

#[test]
fn bound_tier_base_tools_reads_only_the_tier_base_ids() {
    let generated = "id: \"ruff\",\ntier_base: true,\nid: \"prettier\",\ntier_base: false,\n";
    assert_eq!(bound_tier_base_tools(generated), ["ruff"]);
}

#[test]
fn every_runner_tier_base_tool_has_a_bound_tier_base() {
    let runner = runner_tier_base_tools(&read_runfiles("core/quality/runner/src/real/staging.rs"));
    assert!(!runner.is_empty(), "no tier base tools parsed");
    let bound = bound_tier_base_tools(&read_runfiles("cli/generate/src/tools_gen.rs"));
    let unbound: Vec<&String> = runner.iter().filter(|tool| !bound.contains(tool)).collect();
    assert!(
        unbound.is_empty(),
        "TIER_BASE_TOOLS in core/quality/runner/src/real/staging.rs names tools without \
         tier_base in core/quality tool config metadata: {unbound:?}"
    );
}

#[test]
fn the_tools_the_runner_refuses_without_a_config_are_the_tools_it_cannot_default() {
    let real_dir = runfiles_root().join("core/quality/runner/src/real");
    let mut sources: Vec<_> = std::fs::read_dir(&real_dir)
        .expect("runner real sources directory")
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    sources.sort();
    let joined: String = sources
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("runner source"))
        .collect();
    let refusals = runner_config_refusals(&joined);
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

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn dx_dir_name() -> String {
    const SOURCE: &str = "cli/path/src/state.rs";
    const PREFIX: &str = "pub const DX_DIR: &str = \"";
    read_runfiles(SOURCE)
        .lines()
        .find_map(|line| line.trim().strip_prefix(PREFIX)?.strip_suffix("\";"))
        .unwrap_or_else(|| panic!("{SOURCE} declares no DX_DIR string constant"))
        .to_owned()
}

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
        for name in ["BUILD.bazel", "REPO.bazel"] {
            let file = resolve_runfiles(&format!("{dir}/{name}"));
            assert!(file.is_file(), "{} is not a file", file.display());
        }
        if repo.ends_with("local_config_winsdk") {
            let bzl = read_runfiles(&format!("{dir}/toolchains.bzl"));
            assert!(
                bzl.contains("def register_local_rc_exe_toolchains("),
                "{dir}/toolchains.bzl must define register_local_rc_exe_toolchains"
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
    let lock: serde_json::Value =
        serde_json::from_str(&read_runfiles("core/quality/artifacts/lock.json"))
            .expect("lock.json is JSON");
    let shipped: Vec<&str> = lock["scalafmt"]
        .as_object()
        .expect("lock.json pins scalafmt")
        .values()
        .filter_map(|cell| cell["upstream_version"].as_str())
        .collect();
    for tier in ["standard", "strict"] {
        let path = format!("core/quality/tiers/scalafmt/{tier}.conf");
        let pinned = version_of(&read_runfiles(&path));
        assert_eq!(
            pinned,
            version_of(&read_runfiles(".scalafmt.conf")),
            "{path}"
        );
        assert!(
            shipped.contains(&pinned.as_str()),
            "{path} pins {pinned}, which core/quality/artifacts/lock.json does not ship"
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
            "MODULE.bazel.lock has no {extension} entry for {missing:?}; comment /lock-refresh on the pull request"
        );
        for digest in ["usagesDigest", "bzlTransitiveDigest"] {
            let current = &entries[platforms[0]][digest];
            let stale: Vec<&str> = platforms
                .into_iter()
                .filter(|platform| &entries[platform][digest] != current)
                .collect();
            assert!(
                stale.is_empty(),
                "MODULE.bazel.lock has a stale {extension} entry for {stale:?} ({digest}); comment /lock-refresh on the pull request"
            );
        }
    }
}

fn exec_crate_dirs(build: &str) -> std::collections::BTreeSet<String> {
    build
        .lines()
        .filter_map(|line| line.trim().strip_prefix("\"//cli/exec_"))
        .filter_map(|rest| rest.split_once(':'))
        .map(|(name, _)| format!("cli/exec_{name}"))
        .collect()
}

#[test]
fn the_cli_clippy_configs_extend_the_workspace_one() {
    let workspace = read_runfiles("tools/workspace/clippy.toml");
    let cli = read_runfiles("cli/cli/clippy.toml");
    assert!(
        cli.starts_with(&workspace),
        "cli/cli/clippy.toml must start with tools/workspace/clippy.toml and only append to it"
    );
    let dirs = exec_crate_dirs(&read_runfiles("cli/cli/BUILD.bazel"));
    assert!(
        !dirs.is_empty(),
        "cli/cli/BUILD.bazel names no cli/exec_* crate"
    );
    for dir in dirs {
        let path = format!("{dir}/clippy.toml");
        assert_eq!(
            read_runfiles(&path),
            cli,
            "{path} must equal cli/cli/clippy.toml"
        );
    }
}
