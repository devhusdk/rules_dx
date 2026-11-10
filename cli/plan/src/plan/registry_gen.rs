// Generated from core/quality/real_aspect_labels.bzl.
// Run `bazel run //cli/plan:update_registry_gen` to refresh it.

pub const DOCS_ASPECTS: &[&str] = &[
    "tools/docs:aspect.bzl%real_docs_aspect",
];

pub const FORMAT_ASPECTS: &[&str] = &[
    "core/quality:families/cue.bzl%real_cue_format_aspect",
    "core/quality:families/core.bzl%real_format_aspect",
    "core/quality:families/jsonnet.bzl%real_jsonnet_format_aspect",
    "cmake/private/quality:cmake.bzl%real_cmake_format_aspect",
    "make/private/quality:make.bzl%real_make_format_aspect",
    "core/quality:families/nix.bzl%real_nix_format_aspect",
    "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_format_aspect",
    "core/quality:families/shell.bzl%real_shell_format_aspect",
    "core/quality:families/typst.bzl%real_typst_format_aspect",
    "core/quality:families/latex.bzl%real_latex_format_aspect",
    "core/quality:families/html_template.bzl%real_html_template_format_aspect",
    "core/quality:families/yaml.bzl%real_yaml_format_aspect",
];

pub const LINT_ASPECTS: &[&str] = &[
    "dockerfile/private/quality:dockerfile.bzl%real_dockerfile_lint_aspect",
    "core/quality:families/oci_image.bzl%real_oci_image_lint_aspect",
    "make/private/quality:make.bzl%real_make_lint_aspect",
    "cmake/private/quality:cmake.bzl%real_cmake_lint_aspect",
    "core/quality:families/core.bzl%real_lint_aspect",
    "core/quality:families/shell.bzl%real_shell_lint_aspect",
    "core/quality:families/text.bzl%real_text_lint_aspect",
    "core/quality:families/html_template.bzl%real_html_template_lint_aspect",
    "core/quality:families/yaml.bzl%real_yaml_lint_aspect",
    "core/quality:families/nix.bzl%real_nix_lint_aspect",
    "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_lint_aspect",
    "ansible/private/quality:ansible.bzl%real_ansible_lint_aspect",
    "cloudformation/private/quality:cloudformation.bzl%real_cloudformation_lint_aspect",
    "core/quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
    "core/quality:families/typst.bzl%real_typst_lint_aspect",
    "core/quality:families/xml.bzl%real_xml_lint_aspect",
    "core/quality:families/latex.bzl%real_latex_lint_aspect",
];

pub const TYPECHECK_ASPECTS: &[&str] = &[
    "core/quality:families/core.bzl%real_typecheck_aspect",
    "core/quality:families/cue.bzl%real_cue_typecheck_aspect",
    "core/quality:families/oci_image.bzl%real_oci_image_typecheck_aspect",
];

pub const FORMAT_PROBES: &[&str] = &[
    "core/quality:families/core_probe.bzl%real_core_format_probe_aspect",
];

pub const LINT_PROBES: &[&str] = &[
    "core/quality:families/core_probe.bzl%real_core_lint_probe_aspect",
];

pub const TYPECHECK_PROBES: &[&str] = &[
    "core/quality:families/core_probe.bzl%real_core_typecheck_probe_aspect",
];
