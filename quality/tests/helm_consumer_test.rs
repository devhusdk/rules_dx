use std::path::Path;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const KUBECONFORM_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_kubeconform_linux_x86_64";
const HELM_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_helm_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "helm_family",
    family_id = "helm",
    disabled = [
        "audit",
        "typecheck",
        "format",
    ],
    lint = ["helm"],
)

workspace_policy(
    name = "policy",
    families = [":helm_family"],
)
"#;

const SCHEMAS_BUILD: &str = r#"load("@rules_dx//kubernetes/rules:defs.bzl", "kubernetes_schemas")

package(default_visibility = ["//visibility:public"])

kubernetes_schemas(
    name = "schemas",
    kubernetes_version = "1.32.0",
    schemas = ["configmap-v1.json"],
)
"#;

const CONFIGMAP: &str = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: {{ .Release.Name }}\ndata:\n  key: {{ .Values.key }}\n";

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
    write(&root.join("schemas/BUILD.bazel"), SCHEMAS_BUILD);
    write(
        &root.join("schemas/configmap-v1.json"),
        &read(
            &rules
                .join("quality/testdata/kubernetes_schemas")
                .join("configmap-v1.json"),
        ),
    );
    chart(&root, "clean", &Chart::default());
    chart(
        &root,
        "vendored",
        &Chart {
            dependency: true,
            vendored: true,
            ..Chart::default()
        },
    );
    root
}

struct Chart<'a> {
    template: &'a str,
    values: &'a str,
    override_values: &'a str,
    hint: bool,
    dependency: bool,
    vendored: bool,
}

impl Default for Chart<'_> {
    fn default() -> Self {
        Chart {
            template: CONFIGMAP,
            values: "key: 1\n",
            override_values: "key: ok\n",
            hint: true,
            dependency: false,
            vendored: false,
        }
    }
}

fn chart(root: &Path, name: &str, chart: &Chart<'_>) {
    let dir = root.join(name);
    let mut metadata = format!("apiVersion: v2\nname: {name}-chart\nversion: 0.1.0\n");
    if chart.dependency {
        metadata.push_str("dependencies:\n  - name: dep\n    version: 0.1.0\n");
    }
    write(&dir.join("Chart.yaml"), &metadata);
    write(&dir.join("values.yaml"), chart.values);
    write(&dir.join("values-prod.yaml"), chart.override_values);
    write(&dir.join("templates/configmap.yaml"), chart.template);
    let hint = if chart.hint {
        "    aspect_hints = [\"//schemas:schemas\"],\n"
    } else {
        ""
    };
    let dependencies = if chart.vendored {
        let archive = rules_dx_source().join("quality/testdata/helm/dep-0.1.0.tgz");
        std::fs::create_dir_all(dir.join("charts")).expect("charts dir");
        std::fs::copy(&archive, dir.join("charts/dep-0.1.0.tgz")).expect("copy archive");
        "    dependencies = [\"charts/dep-0.1.0.tgz\"],\n"
    } else {
        ""
    };
    write(
        &dir.join("BUILD.bazel"),
        &format!(
            "load(\"@rules_dx//helm/rules:defs.bzl\", \"helm_chart\")\n\nhelm_chart(\n    name = \"chart\",\n{hint}{dependencies}    srcs = [\n        \"Chart.yaml\",\n        \"templates/configmap.yaml\",\n        \"values.yaml\",\n    ],\n    values = [\"values-prod.yaml\"],\n    release_name = \"demo\",\n)\n"
        ),
    );
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("helm-home");
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

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn a_clean_chart_with_a_values_override_and_a_vendored_dependency_passes() {
    let root = consumer();
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_wrong_field_type_in_a_rendered_template_is_an_error_on_that_template() {
    let root = consumer();
    chart(
        &root,
        "broken",
        &Chart {
            override_values: "key: 2\n",
            ..Chart::default()
        },
    );
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("broken/templates/configmap.yaml"),
        "{}",
        out.combined()
    );
    assert!(
        !out.combined().contains("clean/templates"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_render_error_reports_the_template_position() {
    let root = consumer();
    chart(
        &root,
        "renderfail",
        &Chart {
            template: "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: {{ .Values.a.b }}\n",
            ..Chart::default()
        },
    );
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined()
            .contains("renderfail/templates/configmap.yaml:"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_declared_dependency_without_a_vendored_archive_is_an_error_not_clean() {
    let root = consumer();
    chart(
        &root,
        "missingdep",
        &Chart {
            dependency: true,
            ..Chart::default()
        },
    );
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.combined().contains("missingdep"), "{}", out.combined());
}

#[test]
fn a_chart_without_the_schemas_hint_names_kubernetes_schemas() {
    let root = consumer();
    chart(
        &root,
        "unbound",
        &Chart {
            hint: false,
            ..Chart::default()
        },
    );
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("kubernetes_schemas"),
        "{}",
        out.combined()
    );
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    chart(
        &root,
        "broken",
        &Chart {
            override_values: "key: 2\n",
            ..Chart::default()
        },
    );
    let files = [
        "broken/Chart.yaml",
        "broken/values.yaml",
        "broken/values-prod.yaml",
        "broken/templates/configmap.yaml",
    ];
    let before: Vec<String> = files.iter().map(|file| read(&root.join(file))).collect();
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    let after: Vec<String> = files.iter().map(|file| read(&root.join(file))).collect();
    assert_eq!(before, after);
}

#[test]
fn the_declared_helm_and_kubeconform_are_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let fresh = root.bazel(&["clean", "--expunge"]);
    assert!(fresh.status.success(), "{}", fresh.combined());
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("helm-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    chart(
        &root,
        "broken",
        &Chart {
            override_values: "key: 2\n",
            ..Chart::default()
        },
    );
    let built = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("broken/templates/configmap.yaml"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    for repo in [HELM_REPO, KUBECONFORM_REPO] {
        let offline = root.bazel(&["fetch", repo, denied_flag.as_str()]);
        assert!(offline.status.success(), "{}", offline.combined());
    }
}
