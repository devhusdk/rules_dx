use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const SITE: &str = "---\n- name: Site\n  hosts: all\n  gather_facts: false\n  tasks:\n    - name: Report readiness\n      ansible.builtin.debug:\n        msg: ready\n";

const OTHER: &str = "---\n- name: Other\n  hosts: all\n  gather_facts: false\n  tasks:\n    - name: Report other\n      ansible.builtin.debug:\n        msg: other\n";

const ROLE_TASKS: &str = "---\n- name: Install marker\n  ansible.builtin.debug:\n    msg: role\n";

const UNNAMED: &str = "---\n- name: Unnamed task\n  hosts: all\n  gather_facts: false\n  tasks:\n    - ansible.builtin.debug:\n        msg: ready\n";

const TRUTHY: &str = "---\n- name: Truthy\n  hosts: all\n  gather_facts: false\n  become: yes\n  tasks:\n    - name: Report readiness\n      ansible.builtin.debug:\n        msg: ready\n";

const COLLECTION_USE: &str = "---\n- name: Greet\n  hosts: all\n  gather_facts: false\n  tasks:\n    - name: Say hello\n      dxtest.tiny.hello:\n";

const BROKEN: &str = "---\n- name: Broken\n  hosts: all\n  tasks: [unclosed\n";

const RELAXED: &str = "---\nskip_list:\n  - name[missing]\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.10")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")

ansible = use_extension("@rules_dx//ansible:extensions.bzl", "ansible")
ansible.collections(name = "ansible_deps", lock = "//:ansible_collections.lock.json")
use_repo(ansible, "ansible_deps")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "ansible_lint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "ansible_family",
    family_id = "ansible",
    disabled = [
        "format",
        "typecheck",
    ],
    lint = ["ansible_lint"],
)

workspace_policy(
    name = "policy",
    families = [":ansible_family"],
)

ansible_lint_config(
    name = "relaxed",
    src = "relaxed.ansible-lint",
    visibility = ["//visibility:public"],
)

ansible_lint_config(
    name = "with_collections",
    src = "relaxed.ansible-lint",
    data = ["@ansible_deps//:collections"],
    visibility = ["//visibility:public"],
)
"#;

const PLAY_BUILD: &str = r#"load("@rules_dx//ansible/rules:defs.bzl", "ansible_library")

ansible_library(
    name = "play",
    aspect_hints = HINTS,
    srcs = [
        "playbooks/other.yml",
        "playbooks/site.yml",
        "roles/web/tasks/main.yml",
    ],
)
"#;

const COLLECTION_BUILD: &str = r#"load("@rules_dx//ansible/rules:defs.bzl", "ansible_library")

ansible_library(
    name = "greet",
    aspect_hints = ["//:with_collections"],
    srcs = ["playbooks/greet.yml"],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("play/BUILD.bazel"),
        &PLAY_BUILD.replace("HINTS", hints),
    );
}

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("relaxed.ansible-lint"), RELAXED);
    let fixtures = rules.join("ansible/tests/fixtures/collections");
    write(
        &root.join("ansible_collections.lock.json"),
        &read(&fixtures.join("lock.json")),
    );
    std::fs::copy(
        fixtures.join("dxtest-tiny-1.0.0.tar.gz"),
        root.join("dxtest-tiny-1.0.0.tar.gz"),
    )
    .expect("copy collection archive");
    set_hints(&root, "[]");
    write(&root.join("play/playbooks/site.yml"), SITE);
    write(&root.join("play/playbooks/other.yml"), OTHER);
    write(&root.join("play/roles/web/tasks/main.yml"), ROLE_TASKS);
    write(&root.join("greet/BUILD.bazel"), COLLECTION_BUILD);
    write(&root.join("greet/playbooks/greet.yml"), COLLECTION_USE);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("ansible-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn lint(root: &Case) -> Run {
    dx_with(root, "lint", &[], &[])
}

#[test]
fn playbooks_and_roles_with_a_collection_lock_lint_clean() {
    let root = consumer();
    let clean = lint(&root);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
}

#[test]
fn a_violation_is_reported_and_check_leaves_files_unchanged() {
    let root = consumer();
    write(&root.join("play/playbooks/site.yml"), UNNAMED);
    let dirty = lint(&root);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("play/playbooks/site.yml"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&root.join("play/playbooks/site.yml")), UNNAMED);
    assert_eq!(read(&root.join("play/playbooks/other.yml")), OTHER);
    assert_eq!(
        read(&root.join("play/roles/web/tasks/main.yml")),
        ROLE_TASKS
    );
}

#[test]
fn a_bound_non_default_config_changes_the_result() {
    let root = consumer();
    write(&root.join("play/playbooks/site.yml"), UNNAMED);
    let before = lint(&root);
    assert_eq!(before.status.code(), Some(1), "{}", before.combined());
    set_hints(&root, "[\"//:relaxed\"]");
    let after = lint(&root);
    assert_eq!(after.status.code(), Some(0), "{}", after.combined());
}

#[test]
fn an_unbound_collection_is_a_violation_and_the_bound_lock_resolves_it() {
    let root = consumer();
    write(
        &root.join("greet/BUILD.bazel"),
        &COLLECTION_BUILD.replace("//:with_collections", "//:relaxed"),
    );
    let missing = lint(&root);
    assert_ne!(missing.status.code(), Some(0), "{}", missing.combined());
    write(&root.join("greet/BUILD.bazel"), COLLECTION_BUILD);
    let bound = lint(&root);
    assert_eq!(bound.status.code(), Some(0), "{}", bound.combined());
}

#[test]
fn a_syntax_error_is_never_clean() {
    let root = consumer();
    write(&root.join("play/playbooks/site.yml"), BROKEN);
    let broken = lint(&root);
    assert_ne!(broken.status.code(), Some(0), "{}", broken.combined());
    assert_eq!(read(&root.join("play/playbooks/site.yml")), BROKEN);
}

#[test]
fn apply_rewrites_only_owned_files_with_fixes_and_recheck_passes() {
    let root = consumer();
    write(&root.join("play/playbooks/site.yml"), TRUTHY);
    let dirty = lint(&root);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert_eq!(read(&root.join("play/playbooks/site.yml")), TRUTHY);
    let applied = dx_with(&root, "lint", &["--apply"], &[]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(read(&root.join("play/playbooks/site.yml")).contains("become: true"));
    assert_eq!(read(&root.join("play/playbooks/other.yml")), OTHER);
    assert_eq!(
        read(&root.join("play/roles/web/tasks/main.yml")),
        ROLE_TASKS
    );
    let again = lint(&root);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn the_declared_ansible_lint_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = lint(&root);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("ansible-denied");
    let denied = scratch_dir.path().join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("play/playbooks/site.yml"), UNNAMED);
    let built = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("play/playbooks/site.yml"),
        "{}",
        built.combined()
    );
}
