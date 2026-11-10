// Generated from quality/real_aspect_labels.bzl.
// Run `bazel run //cli/plan:update_registry_gen` to refresh it.

pub const DOCS_ASPECTS: &[&str] = &[
    "tools/docs:aspect.bzl%real_docs_aspect",
];

pub const FORMAT_ASPECTS: &[&str] = &[
    "quality:families/cue.bzl%real_cue_format_aspect",
    "quality:families/core.bzl%real_format_aspect",
    "quality:families/jsonnet.bzl%real_jsonnet_format_aspect",
    "quality:families/bicep.bzl%real_bicep_format_aspect",
    "quality:families/cmake.bzl%real_cmake_format_aspect",
    "quality:families/make.bzl%real_make_format_aspect",
    "quality:families/nix.bzl%real_nix_format_aspect",
    "quality:families/kubernetes.bzl%real_kubernetes_format_aspect",
    "quality:families/rego.bzl%real_rego_format_aspect",
    "quality:families/shell.bzl%real_shell_format_aspect",
    "quality:families/terraform.bzl%real_terraform_format_aspect",
    "quality:families/typst.bzl%real_typst_format_aspect",
    "quality:families/latex.bzl%real_latex_format_aspect",
    "quality:families/html_template.bzl%real_html_template_format_aspect",
    "quality:families/sql.bzl%real_sql_format_aspect",
    "quality:families/yaml.bzl%real_yaml_format_aspect",
];

pub const LINT_ASPECTS: &[&str] = &[
    "quality:families/dockerfile.bzl%real_dockerfile_lint_aspect",
    "quality:families/make.bzl%real_make_lint_aspect",
    "quality:families/cmake.bzl%real_cmake_lint_aspect",
    "quality:families/terraform.bzl%real_terraform_lint_aspect",
    "quality:families/core.bzl%real_lint_aspect",
    "quality:families/shell.bzl%real_shell_lint_aspect",
    "quality:families/text.bzl%real_text_lint_aspect",
    "quality:families/html_template.bzl%real_html_template_lint_aspect",
    "quality:families/yaml.bzl%real_yaml_lint_aspect",
    "quality:families/nix.bzl%real_nix_lint_aspect",
    "quality:families/kubernetes.bzl%real_kubernetes_lint_aspect",
    "quality:families/ansible.bzl%real_ansible_lint_aspect",
    "quality:families/cloudformation.bzl%real_cloudformation_lint_aspect",
    "quality:families/bicep.bzl%real_bicep_lint_aspect",
    "quality:families/sql.bzl%real_sql_lint_aspect",
    "quality:families/rego.bzl%real_rego_lint_aspect",
    "quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
    "quality:families/typst.bzl%real_typst_lint_aspect",
    "quality:families/xml.bzl%real_xml_lint_aspect",
    "quality:families/latex.bzl%real_latex_lint_aspect",
];

pub const TYPECHECK_ASPECTS: &[&str] = &[
    "quality:families/core.bzl%real_typecheck_aspect",
    "quality:families/rego.bzl%real_rego_typecheck_aspect",
    "quality:families/cue.bzl%real_cue_typecheck_aspect",
    "quality:families/terraform.bzl%real_terraform_typecheck_aspect",
];

pub const FORMAT_PROBES: &[&str] = &[
    "quality:families/core_probe.bzl%real_core_format_probe_aspect",
];

pub const LINT_PROBES: &[&str] = &[
    "quality:families/core_probe.bzl%real_core_lint_probe_aspect",
];

pub const TYPECHECK_PROBES: &[&str] = &[
    "quality:families/core_probe.bzl%real_core_typecheck_probe_aspect",
];
