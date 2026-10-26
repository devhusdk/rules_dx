use super::*;

const REGISTRY: &str = "https://bcr.bazel.build";
const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ARCHIVE_SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn no_includes(label: &str) -> Result<String, String> {
    Err(format!("no include {label}"))
}

fn inputs_of(module: &str, lock_text: &str) -> Result<Vec<BazelInput>, String> {
    parse_bazel_inputs(module, lock_text, &no_includes)
}

fn modules_of(module: &str, lock_text: &str) -> Result<Vec<LockedPackage>, String> {
    Ok(inputs_of(module, lock_text)?
        .iter()
        .map(BazelInput::locked)
        .collect())
}

fn lock(modules: &[(&str, &str)]) -> String {
    let mut hashes = serde_json::Map::new();
    hashes.insert(
        format!("{REGISTRY}/bazel_registry.json"),
        serde_json::json!(DIGEST_B),
    );
    for (name, version) in modules {
        hashes.insert(
            format!("{REGISTRY}/modules/{name}/{version}/MODULE.bazel"),
            serde_json::json!(DIGEST_B),
        );
        hashes.insert(
            format!("{REGISTRY}/modules/{name}/{version}/source.json"),
            serde_json::json!(DIGEST_A),
        );
    }
    serde_json::json!({"lockFileVersion": 28, "registryFileHashes": hashes}).to_string()
}

#[test]
fn registry_modules_come_from_source_json_entries_with_origin_and_digest() {
    let module = "bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";
    let inputs = inputs_of(
        module,
        &lock(&[("rules_cc", "0.2.1"), ("platforms", "1.0.0")]),
    )
    .expect("parses");
    let names: Vec<(&str, &str)> = inputs
        .iter()
        .map(|input| (input.name.as_str(), input.version.as_str()))
        .collect();
    assert_eq!(names, vec![("platforms", "1.0.0"), ("rules_cc", "0.2.1")]);
    let cc = &inputs[1];
    assert_eq!(cc.source, BazelSource::Registry);
    assert_eq!(cc.origin, REGISTRY);
    assert_eq!(
        cc.digest.as_deref(),
        Some(format!("source.json sha256:{DIGEST_A}").as_str())
    );
    assert_eq!(
        cc.provenance(),
        format!("registry {REGISTRY} source.json sha256:{DIGEST_A}")
    );
}

#[test]
fn a_module_fetched_but_not_selected_is_not_inventoried() {
    let lock_text = lock(&[("rules_cc", "0.2.1")]);
    let mut root: serde_json::Value = serde_json::from_str(&lock_text).expect("json");
    root["registryFileHashes"][format!("{REGISTRY}/modules/rules_cc/0.1.0/MODULE.bazel")] =
        serde_json::json!(DIGEST_B);
    let inputs = inputs_of("", &root.to_string()).expect("parses");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].version, "0.2.1");
}

#[test]
fn locked_packages_are_bazel_set_members_with_provenance_location() {
    let packages = modules_of("", &lock(&[("platforms", "1.0.0")])).expect("parses");
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].set, "bazel");
    assert!(!packages[0].is_git && !packages[0].is_private);
    assert!(packages[0]
        .location
        .as_deref()
        .is_some_and(|text| text.starts_with("registry https://bcr.bazel.build")));
}

#[test]
fn a_declared_dep_missing_from_the_lock_fails() {
    let module = "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n";
    let error = inputs_of(module, &lock(&[("platforms", "1.0.0")])).expect_err("fails");
    assert!(
        error.contains("bazel_dep rules_go is not selected"),
        "{error}"
    );
}

#[test]
fn a_declared_version_above_the_selected_version_fails() {
    let module = "bazel_dep(name = \"rules_cc\", version = \"0.3.0\")\n";
    let error = inputs_of(module, &lock(&[("rules_cc", "0.2.1")])).expect_err("fails");
    assert!(error.contains("declares 0.3.0"), "{error}");
    assert!(error.contains("selects 0.2.1"), "{error}");
}

#[test]
fn a_higher_selected_version_is_accepted() {
    let module = "bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";
    inputs_of(module, &lock(&[("rules_cc", "0.2.1")])).expect("parses");
}

#[test]
fn archive_override_replaces_the_registry_identity_with_the_archive_facts() {
    let module = r#"
bazel_dep(name = "rules_foo", version = "1.2.3")
archive_override(
    module_name = "rules_foo",
    urls = ["https://example.com/rules_foo.tar.gz", "https://mirror.example.com/rules_foo.tar.gz"],
    integrity = "sha256-AAAA",
    patches = ["//patches:fix.patch"],
)
"#;
    let inputs = inputs_of(module, &lock(&[])).expect("parses");
    assert_eq!(inputs.len(), 1);
    let input = &inputs[0];
    assert_eq!(input.name, "rules_foo");
    assert_eq!(input.version, "1.2.3");
    assert_eq!(input.source, BazelSource::ArchiveOverride);
    assert_eq!(input.origin, "https://example.com/rules_foo.tar.gz");
    assert_eq!(input.digest.as_deref(), Some("sha256-AAAA"));
    assert!(input.patched);
    assert!(input.provenance().ends_with(" patched"));
}

#[test]
fn git_override_is_a_git_package_pinned_to_its_commit() {
    let module = r#"
bazel_dep(name = "rules_bar", version = "0.1.0")
git_override(module_name = "rules_bar", remote = "https://example.com/rules_bar.git", commit = "0123456789abcdef")
"#;
    let packages = modules_of(module, &lock(&[])).expect("parses");
    assert_eq!(packages.len(), 1);
    assert!(packages[0].is_git);
    assert_eq!(packages[0].version, "0123456789abcdef");
    assert!(packages[0]
        .location
        .as_deref()
        .is_some_and(|text| text.contains("https://example.com/rules_bar.git")));
}

#[test]
fn git_override_without_a_commit_fails() {
    let module = r#"
bazel_dep(name = "rules_bar", version = "0.1.0")
git_override(module_name = "rules_bar", remote = "https://example.com/rules_bar.git", branch = "main")
"#;
    let error = inputs_of(module, &lock(&[])).expect_err("fails");
    assert!(error.contains("not pinned to a commit"), "{error}");
}

#[test]
fn local_path_override_is_a_private_package() {
    let module = r#"
bazel_dep(name = "rules_baz", version = "0.0.0")
local_path_override(module_name = "rules_baz", path = "../rules_baz")
"#;
    let packages = modules_of(module, &lock(&[])).expect("parses");
    assert_eq!(packages.len(), 1);
    assert!(packages[0].is_private);
    assert!(!packages[0].is_git);
}

#[test]
fn an_override_that_the_lock_still_resolves_from_a_registry_fails() {
    let module = r#"
bazel_dep(name = "rules_foo", version = "1.0.0")
local_path_override(module_name = "rules_foo", path = "../rules_foo")
"#;
    let error = inputs_of(module, &lock(&[("rules_foo", "1.0.0")])).expect_err("fails");
    assert!(
        error.contains("overrides it with local_path_override"),
        "{error}"
    );
}

#[test]
fn single_version_override_marks_the_registry_module_patched() {
    let module = r#"
bazel_dep(name = "rules_cc", version = "0.2.0")
single_version_override(module_name = "rules_cc", patches = ["//p:a.patch"], patch_strip = 1)
"#;
    let inputs = inputs_of(module, &lock(&[("rules_cc", "0.2.0")])).expect("parses");
    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].patched);
    assert_eq!(inputs[0].source, BazelSource::Registry);
}

#[test]
fn an_archive_override_without_a_digest_fails() {
    let module = r#"
bazel_dep(name = "rules_foo", version = "1.0.0")
archive_override(module_name = "rules_foo", urls = ["https://example.com/a.tar.gz"])
"#;
    let error = inputs_of(module, &lock(&[])).expect_err("fails");
    assert!(error.contains("no sha256 or integrity"), "{error}");
}

#[test]
fn a_computed_override_module_name_fails() {
    let module = "archive_override(module_name = NAME, urls = [\"https://e/a\"], integrity = \"sha256-A\")\n";
    let error = inputs_of(module, &lock(&[])).expect_err("fails");
    assert!(error.contains("computed module_name"), "{error}");
}

#[test]
fn http_archives_declared_through_use_repo_rule_are_inventoried_by_digest() {
    let module = format!(
        r#"
_http_archive = use_repo_rule("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
_custom = use_repo_rule("//tools:repos.bzl", "custom_repo")
_http_archive(
    name = "tool_linux",
    urls = ["https://example.com/tool-1.0.tar.gz"],
    sha256 = "{ARCHIVE_SHA}",
)
_custom(name = "not_an_archive")
"#
    );
    let inputs = inputs_of(&module, &lock(&[])).expect("parses");
    assert_eq!(inputs.len(), 2);
    let input = inputs
        .iter()
        .find(|input| input.name == "tool_linux")
        .expect("tool_linux");
    assert_eq!(input.source, BazelSource::HttpArchive);
    assert_eq!(input.origin, "https://example.com/tool-1.0.tar.gz");
    assert_eq!(input.version, format!("sha256:{ARCHIVE_SHA}"));
}

#[test]
fn an_http_archive_without_a_digest_fails() {
    let module = r#"
_http_archive = use_repo_rule("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
_http_archive(name = "floating", urls = ["https://example.com/a.tar.gz"])
"#;
    let error = inputs_of(module, &lock(&[])).expect_err("fails");
    assert!(error.contains("no sha256 or integrity"), "{error}");
}

#[test]
fn a_malformed_sha256_fails() {
    let module = r#"
_http_archive = use_repo_rule("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
_http_archive(name = "short", urls = ["https://example.com/a.tar.gz"], sha256 = "abc")
"#;
    let error = inputs_of(module, &lock(&[])).expect_err("fails");
    assert!(error.contains("64 lowercase hex"), "{error}");
}

#[test]
fn a_lock_without_registry_hashes_fails() {
    let error = inputs_of("", "{\"lockFileVersion\": 28}").expect_err("fails");
    assert!(error.contains("missing registryFileHashes"), "{error}");
    let error = inputs_of("", "{}").expect_err("fails");
    assert!(error.contains("missing lockFileVersion"), "{error}");
    let error = inputs_of("", "not json").expect_err("fails");
    assert!(error.starts_with("invalid MODULE.bazel.lock"), "{error}");
}

#[test]
fn a_source_entry_with_a_missing_digest_or_odd_shape_fails() {
    let text = serde_json::json!({
        "lockFileVersion": 28,
        "registryFileHashes": {format!("{REGISTRY}/modules/a/1.0/source.json"): null}
    })
    .to_string();
    let error = inputs_of("", &text).expect_err("fails");
    assert!(error.contains("no sha256 digest"), "{error}");
    let text = serde_json::json!({
        "lockFileVersion": 28,
        "registryFileHashes": {format!("{REGISTRY}/modules/a/source.json"): DIGEST_A}
    })
    .to_string();
    let error = inputs_of("", &text).expect_err("fails");
    assert!(
        error.contains("<registry>/modules/<name>/<version>/source.json"),
        "{error}"
    );
}

#[test]
fn module_syntax_errors_fail_closed() {
    let error = inputs_of("bazel_dep(", &lock(&[])).expect_err("fails");
    assert!(error.contains("does not parse"), "{error}");
}

#[test]
fn duplicate_deps_and_overrides_fail() {
    let module =
        "bazel_dep(name = \"a\", version = \"1\")\nbazel_dep(name = \"a\", version = \"1\")\n";
    assert!(inputs_of(module, &lock(&[("a", "1")])).is_err());
    let module = r#"
local_path_override(module_name = "a", path = "x")
local_path_override(module_name = "a", path = "y")
"#;
    assert!(inputs_of(module, &lock(&[])).is_err());
}

#[test]
fn version_order_follows_bazel_rules() {
    assert_eq!(bazel_version_cmp("1.9.0", "1.10.0"), Ordering::Less);
    assert_eq!(bazel_version_cmp("1.0", "1.0.0"), Ordering::Less);
    assert_eq!(bazel_version_cmp("1.0.0-rc1", "1.0.0"), Ordering::Less);
    assert_eq!(bazel_version_cmp("1.0.0+build", "1.0.0"), Ordering::Equal);
    assert_eq!(
        bazel_version_cmp("20230802.0", "20230802.0.bcr.1"),
        Ordering::Less
    );
    assert_eq!(bazel_version_cmp("1.0", ""), Ordering::Less);
    assert_eq!(bazel_version_cmp("", ""), Ordering::Equal);
    assert_eq!(bazel_version_cmp("1.2.3", "1.2.3"), Ordering::Equal);
}

#[test]
fn custom_repo_rules_are_inventoried_as_repo_rule_inputs() {
    let module = r#"
_git = use_repo_rule("@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository")
_git(name = "tool_src", remote = "https://example.com/tool.git", commit = "abc123")
_custom = use_repo_rule("//tools:repos.bzl", "custom_repo")
_custom(name = "loose")
"#;
    let inputs = inputs_of(module, &lock(&[])).expect("parses");
    assert_eq!(inputs.len(), 2);
    assert!(inputs
        .iter()
        .all(|input| input.source == BazelSource::RepoRule));
    let git = inputs
        .iter()
        .find(|input| input.name == "tool_src")
        .expect("git");
    assert_eq!(git.origin, "https://example.com/tool.git");
    assert_eq!(git.digest.as_deref(), Some("commit:abc123"));
    let loose = inputs
        .iter()
        .find(|input| input.name == "loose")
        .expect("loose");
    assert_eq!(loose.version, "unpinned");
    assert_eq!(loose.origin, "//tools:repos.bzl custom_repo");
}

#[test]
fn included_segments_are_read() {
    let module = "include(\"//:deps.MODULE.bazel\")\n";
    let read = |label: &str| -> Result<String, String> {
        assert_eq!(label, "//:deps.MODULE.bazel");
        Ok("bazel_dep(name = \"rules_bar\", version = \"0.1.0\")\ngit_override(module_name = \"rules_bar\", remote = \"https://example.com/bar.git\", commit = \"abc123\")\n".to_owned())
    };
    let inputs = parse_bazel_inputs(module, &lock(&[]), &read).expect("parses");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "rules_bar");
    assert_eq!(inputs[0].source, BazelSource::GitOverride);
}

#[test]
fn an_unreadable_or_repeated_include_fails() {
    let error = inputs_of("include(\"//:deps.MODULE.bazel\")\n", &lock(&[])).expect_err("fails");
    assert!(error.contains("no include //:deps.MODULE.bazel"), "{error}");
    let read = |_: &str| -> Result<String, String> { Ok(String::new()) };
    let twice = "include(\"//:a.MODULE.bazel\")\ninclude(\"//:a.MODULE.bazel\")\n";
    let error = parse_bazel_inputs(twice, &lock(&[]), &read).expect_err("fails");
    assert!(error.contains("more than once"), "{error}");
}

#[test]
fn an_include_of_a_repo_label_is_rejected() {
    let read = |label: &str| -> Result<String, String> { panic!("read {label}") };
    let error = parse_bazel_inputs("include(\"@//:deps.MODULE.bazel\")\n", &lock(&[]), &read)
        .expect_err("fails");
    assert!(
        error.contains("include @//:deps.MODULE.bazel is not a //package:file label")
            && error.contains("MODULE.bazel line 1"),
        "{error}"
    );
}

#[test]
fn a_single_version_override_pin_sets_the_declared_floor() {
    let module = r#"
bazel_dep(name = "rules_cc", version = "2.0")
single_version_override(module_name = "rules_cc", version = "1.5")
"#;
    inputs_of(module, &lock(&[("rules_cc", "1.5")])).expect("parses");
}

#[test]
fn archive_override_accepts_a_single_url_string() {
    let module = format!(
        r#"
bazel_dep(name = "rules_foo", version = "1.2.3")
archive_override(module_name = "rules_foo", urls = "https://example.com/foo.tar.gz", sha256 = "{ARCHIVE_SHA}")
"#
    );
    let inputs = inputs_of(&module, &lock(&[])).expect("parses");
    assert_eq!(inputs[0].origin, "https://example.com/foo.tar.gz");
}

const HTTP_FILE: &str = "@@bazel_tools//tools/build_defs/repo:http.bzl%http_file";
const EXT_A: &str = "@@rules_a+//:ext.bzl%a";
const EXT_B: &str = "@@rules_b+//:ext.bzl%b";

fn lock_with_extensions(extensions: serde_json::Value) -> String {
    let mut root: serde_json::Value = serde_json::from_str(&lock(&[])).expect("json");
    root["moduleExtensions"] = extensions;
    root.to_string()
}

fn general(specs: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"general": {"bzlTransitiveDigest": "x", "generatedRepoSpecs": specs}})
}

fn spec(attributes: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"repoRuleId": HTTP_FILE, "attributes": attributes})
}

fn extension_inputs_of(extensions: serde_json::Value) -> Result<Vec<BazelInput>, String> {
    inputs_of("", &lock_with_extensions(extensions))
}

#[test]
fn extension_repos_with_urls_are_listed_with_origin_and_digest() {
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({
            "tool_jar": spec(serde_json::json!({"urls": ["https://example.com/tool.jar"], "sha256": ARCHIVE_SHA})),
            "custom": {"repoRuleId": "@@rules_a+//:repo.bzl%custom_repo", "attributes": {"url": "https://example.com/custom.tgz", "integrity": "sha256-abc="}},
        })),
    }))
    .expect("parses");
    assert_eq!(inputs.len(), 2);
    let custom = &inputs[0];
    assert_eq!(custom.name, "custom");
    assert_eq!(custom.source, BazelSource::Extension);
    assert_eq!(custom.origin, "https://example.com/custom.tgz");
    assert_eq!(custom.version, "sha256-abc=");
    let jar = &inputs[1];
    assert_eq!(jar.name, "tool_jar");
    assert_eq!(jar.version, format!("sha256:{ARCHIVE_SHA}"));
    assert_eq!(
        jar.provenance(),
        format!("module_extension https://example.com/tool.jar sha256:{ARCHIVE_SHA}")
    );
    assert!(!jar.patched);
}

#[test]
fn an_extension_repo_under_several_platform_keys_is_listed_once() {
    let repo = spec(serde_json::json!({"url": "https://example.com/a.tgz", "sha256": ARCHIVE_SHA}));
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: {
            "os:linux": {"generatedRepoSpecs": {"tool": repo.clone()}},
            "os:macos": {"generatedRepoSpecs": {"tool": repo}},
        },
    }))
    .expect("parses");
    assert_eq!(inputs.len(), 1);
}

#[test]
fn an_extension_hub_without_a_url_is_skipped() {
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({
            "hub": {"repoRuleId": "@@rules_a+//:hub.bzl%hub", "attributes": {"repos": ["a"]}},
            "empty_urls": spec(serde_json::json!({"urls": []})),
        })),
    }))
    .expect("parses");
    assert!(inputs.is_empty(), "{inputs:?}");
}

#[test]
fn a_malformed_extension_digest_fails() {
    for attributes in [
        serde_json::json!({"url": "https://example.com/a.tgz", "sha256": "ABC"}),
        serde_json::json!({"url": "https://example.com/a.tgz", "integrity": "md5-abc"}),
    ] {
        let error = extension_inputs_of(serde_json::json!({
            EXT_A: general(serde_json::json!({"bad": spec(attributes)})),
        }))
        .expect_err("fails");
        assert!(error.contains("invalid MODULE.bazel.lock"), "{error}");
        assert!(error.contains(&format!("{EXT_A} bad")), "{error}");
    }
}

#[test]
fn an_extension_repo_without_a_rule_fails() {
    let error = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({"norule": {"attributes": {"url": "https://example.com/a"}}})),
    }))
    .expect_err("fails");
    assert!(error.contains("norule has no repo rule"), "{error}");
}

#[test]
fn an_extension_repo_without_a_digest_is_unpinned_and_empty_integrity_is_absent() {
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({
            "loose": spec(serde_json::json!({"url": "https://example.com/loose.tgz"})),
            "pinned": spec(serde_json::json!({"url": "https://example.com/p.tgz", "urls": [], "integrity": "", "sha256": ARCHIVE_SHA})),
        })),
    }))
    .expect("parses");
    assert_eq!(inputs[0].name, "loose");
    assert_eq!(inputs[0].version, "unpinned");
    assert_eq!(inputs[0].digest, None);
    assert_eq!(inputs[1].origin, "https://example.com/p.tgz");
    assert_eq!(inputs[1].version, format!("sha256:{ARCHIVE_SHA}"));
}

#[test]
fn an_old_style_extension_spec_names_its_rule_by_file_and_class() {
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({
            "old": {"bzlFile": "@@bazel_tools//tools/build_defs/repo:http.bzl", "ruleClassName": "http_archive", "attributes": {"urls": ["https://example.com/old.tgz"], "sha256": ARCHIVE_SHA}},
        })),
    }))
    .expect("parses");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].origin, "https://example.com/old.tgz");
}

#[test]
fn two_extensions_generating_one_repo_from_different_urls_fail() {
    let error = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({"tool": spec(serde_json::json!({"url": "https://a.example/t.tgz", "sha256": ARCHIVE_SHA}))})),
        EXT_B: general(serde_json::json!({"tool": spec(serde_json::json!({"url": "https://b.example/t.tgz", "sha256": ARCHIVE_SHA}))})),
    }))
    .expect_err("fails");
    assert!(error.contains(EXT_A), "{error}");
    assert!(error.contains(EXT_B), "{error}");
    assert!(error.contains("tool"), "{error}");
}

#[test]
fn an_extension_repo_matching_a_module_download_fails_as_a_duplicate() {
    let module = format!(
        "http_archive = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\nhttp_archive(name = \"tool\", urls = [\"https://a.example/t.tgz\"], sha256 = \"{ARCHIVE_SHA}\")\n"
    );
    let lock_text = lock_with_extensions(serde_json::json!({
        EXT_A: general(serde_json::json!({"tool": spec(serde_json::json!({"url": "https://a.example/t.tgz", "sha256": ARCHIVE_SHA}))})),
    }));
    let error = inputs_of(&module, &lock_text).expect_err("fails");
    assert!(
        error.contains("declared twice across MODULE.bazel and MODULE.bazel.lock"),
        "{error}"
    );
}

#[test]
fn a_lock_without_module_extensions_lists_no_extension_inputs() {
    let lock_text = lock(&[("rules_cc", "0.2.1")]);
    let empty = lock_with_extensions(serde_json::json!({}));
    let mut root: serde_json::Value = serde_json::from_str(&empty).expect("json");
    root["registryFileHashes"] = serde_json::from_str::<serde_json::Value>(&lock_text)
        .expect("json")["registryFileHashes"]
        .clone();
    assert_eq!(
        inputs_of("", &lock_text).expect("parses"),
        inputs_of("", &root.to_string()).expect("parses")
    );
}

#[test]
fn patched_and_commit_pinned_extension_repos_keep_those_facts() {
    let inputs = extension_inputs_of(serde_json::json!({
        EXT_A: general(serde_json::json!({
            "git_repo": {"repoRuleId": "@@bazel_tools//tools/build_defs/repo:git.bzl%git_repository", "attributes": {"remote": "https://example.com/r.git", "commit": "abc123"}},
            "patched": spec(serde_json::json!({"url": "https://example.com/p.tgz", "sha256": ARCHIVE_SHA, "patches": ["//:fix.patch"]})),
        })),
    }))
    .expect("parses");
    assert_eq!(inputs[0].name, "git_repo");
    assert_eq!(inputs[0].origin, "https://example.com/r.git");
    assert_eq!(inputs[0].version, "commit:abc123");
    assert!(!inputs[0].patched);
    assert!(inputs[1].patched);
}

#[test]
fn a_non_object_module_extensions_entry_fails() {
    let error = extension_inputs_of(serde_json::json!({EXT_A: []})).expect_err("fails");
    assert!(error.contains("invalid MODULE.bazel.lock"), "{error}");
}
