use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str =
    "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app\ndata:\n  key: value\n";

const DIRTY: &str = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app\ndata:\n  key: 1\n";

const WIDGET: &str =
    "apiVersion: example.com/v1\nkind: Widget\nmetadata:\n  name: w\nspec:\n  size: 2\n";

const DUPLICATE_KEY: &str =
    "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app\ndata:\n  key: a\n  key: b\n";

const NOT_MANIFEST: &str = "name: not-a-manifest\n";

const KUBECONFORM_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_kubeconform_linux_x86_64";

const SCHEMAS: [&str; 4] = [
    "configmap-v1.json",
    "deployment-apps-v1.json",
    "service-v1.json",
    "widget_v1.json",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "kubernetes_family",
    family_id = "kubernetes",
    disabled = [
        "audit",
        "typecheck",
        "format",
    ],
    lint = ["kubeconform"],
)

workspace_policy(
    name = "policy",
    families = [":kubernetes_family"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//kubernetes/rules:defs.bzl", "kubernetes_manifests", "kubernetes_schemas")

kubernetes_schemas(
    name = "schemas",
    kubernetes_version = "1.32.0",
    schemas = [
        "configmap-v1.json",
        "deployment-apps-v1.json",
        "service-v1.json",
    ],
    crd_schemas = {"widget_v1.json": "example.com"},
)

kubernetes_schemas(
    name = "lenient",
    kubernetes_version = "1.32.0",
    schemas = ["configmap-v1.json"],
    strict = False,
    skip_kinds = ["Widget"],
)

kubernetes_manifests(
    name = "app",
    aspect_hints = [":schemas"],
    srcs = [
        "a.yaml",
        "b.yaml",
        "widget.yaml",
    ],
)

kubernetes_manifests(
    name = "tight",
    aspect_hints = [":schemas"],
    srcs = ["tight.yaml"],
)

kubernetes_manifests(
    name = "loose",
    aspect_hints = [":lenient"],
    srcs = ["loose.yaml"],
)
"#;

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
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    for schema in SCHEMAS {
        let from = rules
            .join("quality/testdata/kubernetes_schemas")
            .join(schema);
        write(&root.join("app").join(schema), &read(&from));
    }
    write(&root.join("app/a.yaml"), CLEAN);
    write(&root.join("app/b.yaml"), CLEAN);
    write(&root.join("app/widget.yaml"), WIDGET);
    write(&root.join("app/loose.yaml"), DUPLICATE_KEY);
    write(&root.join("app/tight.yaml"), CLEAN);
    write(&root.join("docs/plain.yaml"), NOT_MANIFEST);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("kubernetes-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
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

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn clean_manifests_pass_with_a_crd_and_a_lenient_config_and_unowned_yaml_is_ignored() {
    let root = consumer();
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("docs/plain.yaml")), NOT_MANIFEST);
}

#[test]
fn lint_reports_file_and_line_for_an_invalid_manifest_and_never_edits() {
    let root = consumer();
    write(&root.join("app/b.yaml"), DIRTY);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.combined().contains("app/b.yaml"), "{}", out.combined());
    assert!(!out.combined().contains("app/a.yaml"), "{}", out.combined());
    assert_eq!(read(&root.join("app/b.yaml")), DIRTY);
}

#[test]
fn strict_rejects_a_duplicate_key_that_a_lenient_config_accepts() {
    let root = consumer();
    write(&root.join("app/tight.yaml"), DUPLICATE_KEY);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/tight.yaml"),
        "{}",
        out.combined()
    );
    assert!(
        !out.combined().contains("app/loose.yaml"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_document_that_is_not_a_manifest_is_an_error() {
    let root = consumer();
    write(&root.join("app/a.yaml"), NOT_MANIFEST);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("dx/kubernetes-not-manifest"),
        "{}",
        out.combined()
    );
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    let file = root.join("app/a.yaml");
    write(&file, DIRTY);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&file), DIRTY);
}

#[test]
fn the_declared_kubeconform_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let fresh = root.bazel(&["clean", "--expunge"]);
    assert!(fresh.status.success(), "{}", fresh.combined());
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("kubernetes-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("app/a.yaml"), DIRTY);
    let built = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/a.yaml"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", KUBECONFORM_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());
}
