"""Aspect and probe labels of the files that load no language rule set, per command.

Language aspects and probes sit in the `quality` entry of each `<lang>/lang.json`.
"""

REAL_ASPECT_LABELS = {
    "lint": [
        "dockerfile/private/quality:dockerfile.bzl%real_dockerfile_lint_aspect",
        "quality:families/make.bzl%real_make_lint_aspect",
        "quality:families/cmake.bzl%real_cmake_lint_aspect",
        "quality:families/core.bzl%real_lint_aspect",
        "quality:families/shell.bzl%real_shell_lint_aspect",
        "quality:families/text.bzl%real_text_lint_aspect",
        "quality:families/html_template.bzl%real_html_template_lint_aspect",
        "quality:families/yaml.bzl%real_yaml_lint_aspect",
        "quality:families/nix.bzl%real_nix_lint_aspect",
        "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_lint_aspect",
        "ansible/private/quality:ansible.bzl%real_ansible_lint_aspect",
        "cloudformation/private/quality:cloudformation.bzl%real_cloudformation_lint_aspect",
        "quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
        "quality:families/typst.bzl%real_typst_lint_aspect",
        "quality:families/xml.bzl%real_xml_lint_aspect",
        "quality:families/latex.bzl%real_latex_lint_aspect",
    ],
    "typecheck": [
        "quality:families/core.bzl%real_typecheck_aspect",
        "quality:families/cue.bzl%real_cue_typecheck_aspect",
    ],
    "format": [
        "quality:families/cue.bzl%real_cue_format_aspect",
        "quality:families/core.bzl%real_format_aspect",
        "quality:families/jsonnet.bzl%real_jsonnet_format_aspect",
        "quality:families/cmake.bzl%real_cmake_format_aspect",
        "quality:families/make.bzl%real_make_format_aspect",
        "quality:families/nix.bzl%real_nix_format_aspect",
        "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_format_aspect",
        "quality:families/shell.bzl%real_shell_format_aspect",
        "quality:families/typst.bzl%real_typst_format_aspect",
        "quality:families/latex.bzl%real_latex_format_aspect",
        "quality:families/html_template.bzl%real_html_template_format_aspect",
        "quality:families/yaml.bzl%real_yaml_format_aspect",
    ],
    "docs": [
        "tools/docs:aspect.bzl%real_docs_aspect",
    ],
}

REAL_PROBE_LABELS = {
    "lint": [
        "quality:families/core_probe.bzl%real_core_lint_probe_aspect",
    ],
    "typecheck": [
        "quality:families/core_probe.bzl%real_core_typecheck_probe_aspect",
    ],
    "format": [
        "quality:families/core_probe.bzl%real_core_format_probe_aspect",
    ],
}
