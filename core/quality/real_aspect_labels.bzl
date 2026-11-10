"""Aspect and probe labels of the files in `core/quality/families/` and `tools/docs/`, per command.

Language aspects and probes sit in the `quality` entry of each `<lang>/lang.json`.
"""

REAL_ASPECT_LABELS = {
    "lint": [
        "core/quality:families/oci_image.bzl%real_oci_image_lint_aspect",
        "core/quality:families/core.bzl%real_lint_aspect",
        "core/quality:families/shell.bzl%real_shell_lint_aspect",
        "core/quality:families/text.bzl%real_text_lint_aspect",
        "core/quality:families/html_template.bzl%real_html_template_lint_aspect",
        "core/quality:families/yaml.bzl%real_yaml_lint_aspect",
        "core/quality:families/nix.bzl%real_nix_lint_aspect",
        "core/quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
        "core/quality:families/typst.bzl%real_typst_lint_aspect",
        "core/quality:families/xml.bzl%real_xml_lint_aspect",
        "core/quality:families/latex.bzl%real_latex_lint_aspect",
    ],
    "typecheck": [
        "core/quality:families/core.bzl%real_typecheck_aspect",
        "core/quality:families/cue.bzl%real_cue_typecheck_aspect",
        "core/quality:families/oci_image.bzl%real_oci_image_typecheck_aspect",
    ],
    "format": [
        "core/quality:families/cue.bzl%real_cue_format_aspect",
        "core/quality:families/core.bzl%real_format_aspect",
        "core/quality:families/jsonnet.bzl%real_jsonnet_format_aspect",
        "core/quality:families/nix.bzl%real_nix_format_aspect",
        "core/quality:families/shell.bzl%real_shell_format_aspect",
        "core/quality:families/typst.bzl%real_typst_format_aspect",
        "core/quality:families/latex.bzl%real_latex_format_aspect",
        "core/quality:families/html_template.bzl%real_html_template_format_aspect",
        "core/quality:families/yaml.bzl%real_yaml_format_aspect",
    ],
    "docs": [
        "tools/docs:aspect.bzl%real_docs_aspect",
    ],
}

REAL_PROBE_LABELS = {
    "lint": [
        "core/quality:families/core_probe.bzl%real_core_lint_probe_aspect",
    ],
    "typecheck": [
        "core/quality:families/core_probe.bzl%real_core_typecheck_probe_aspect",
    ],
    "format": [
        "core/quality:families/core_probe.bzl%real_core_format_probe_aspect",
    ],
}
