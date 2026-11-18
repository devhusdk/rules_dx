"""Matrix Markdown cases."""

load(":runner_matrix_tests.bzl", "matrix_case")

visibility("private")

_VALE_STYLE_FILES = [
    "Cliches.yml",
    "E-Prime.yml",
    "Illusions.yml",
    "LICENSE",
    "Passive.yml",
    "So.yml",
    "ThereIs.yml",
    "TooWordy.yml",
    "Weasel.yml",
    "meta.json",
]

_VALE_TEST_CONFIG = {
    "toolfile_tools": ["vale", "vale"],
    "toolfile_srcs": [":vale_test.ini", "styles/Dx/Markers.yml"],
    "config_tools": ["vale"],
    "config_files": [":vale_test.ini"],
}

_MARKDOWN_VALE = ["markdown_check", "vale"]
_MARKDOWN_VALE_BINARIES = ["//core/quality/markdown:quality_markdown", "@dx_tools//:vale"]

def _vale_profile_files(profile):
    styles = ["write-good/" + f for f in _VALE_STYLE_FILES]
    ini = ":vale_profile/%s/.vale.ini" % profile
    root = "core/quality/testdata/vale_profile/%s/" % profile
    return {
        "toolfile_tools": ["vale"] * (1 + len(styles)),
        "toolfile_srcs": [ini] + ["//core/quality/tiers/vale:styles/" + s for s in styles],
        "toolfile_rels": [""] + [root + ".dx/config/vale/styles/" + s for s in styles],
        "config_tools": ["vale"],
        "config_files": [ini],
    }

def _vale_profile_case(name, profile, family, path, text):
    extra = {}
    if family == "rst":
        extra = {
            "path_tool_owners": ["vale"],
            "path_tool_names": ["rst2html"],
            "path_tool_binaries": ["//core/quality/tools/python:rst2html"],
        }
    return matrix_case(
        name = name,
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = family,
        generated = {path: text},
        **dict(_vale_profile_files(profile), **extra)
    )

def _clean_tier_case(profile):
    kinds = "//core/quality/tiers/markdown_check:%s.txt" % profile
    files = _vale_profile_files(profile)
    return matrix_case(
        name = "matrix_markdown_lint_clean_" + profile,
        capability = "lint",
        tool = _MARKDOWN_VALE,
        binary = _MARKDOWN_VALE_BINARIES,
        source_class = "markdown",
        generated = {"matrix/clean_%s.md" % profile: "# Clean\n\n```sh\nls\n```\n\n## Part\n"},
        toolfile_tools = files["toolfile_tools"] + ["markdown_check"],
        toolfile_srcs = files["toolfile_srcs"] + [kinds],
        toolfile_rels = files["toolfile_rels"] + [""],
        config_tools = ["vale", "markdown_check"],
        config_files = files["config_files"] + [kinds],
    )

MARKDOWN_CASES = [
    matrix_case(
        name = "matrix_markdown_lint_pass",
        capability = "lint",
        tool = _MARKDOWN_VALE,
        binary = _MARKDOWN_VALE_BINARIES,
        source_class = "markdown",
        src = ":real_clean.md",
        **_VALE_TEST_CONFIG
    ),
    matrix_case(
        name = "matrix_markdown_lint_fail",
        capability = "lint",
        tool = _MARKDOWN_VALE,
        binary = _MARKDOWN_VALE_BINARIES,
        source_class = "markdown",
        generated = {
            "matrix/markdown_dirty.md": "# Matrix dirty\n\n# Again\n",
        },
        **_VALE_TEST_CONFIG
    ),
    matrix_case(
        name = "matrix_markdown_lychee_pass",
        capability = "lint",
        tool = "lychee",
        binary = "@dx_tools//:lychee",
        source_class = "markdown",
        generated = {
            "matrix/lychee_clean/README.md": "See <https://example.com/docs>.\n",
        },
    ),
    matrix_case(
        name = "matrix_markdown_lychee_fail",
        capability = "lint",
        tool = "lychee",
        binary = "@dx_tools//:lychee",
        source_class = "markdown",
        generated = {
            "matrix/lychee_dirty/README.md": "See [gone](missing.md).\n",
        },
    ),
    matrix_case(
        name = "matrix_markdown_sibling_pass",
        capability = "lint",
        tool = _MARKDOWN_VALE,
        binary = _MARKDOWN_VALE_BINARIES,
        source_class = "markdown",
        src = ":sibling_clean.md",
        siblings = [":sibling_license.txt"],
        platforms = ["linux_x86_64"],
        **_VALE_TEST_CONFIG
    ),
    matrix_case(
        name = "matrix_vale_rst_lint_pass",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "rst",
        generated = {
            "matrix/rst_clean.rst": "Title\n=====\n\nA clean paragraph with æøå.\n",
        },
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["rst2html"],
        path_tool_binaries = ["//core/quality/tools/python:rst2html"],
    ),
    matrix_case(
        name = "matrix_vale_rst_lint_fail",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "rst",
        generated = {
            "matrix/rst_dirty.rst": "Title\n=====\n\nSecond line.\nFix this TODO before merging.\n",
        },
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["rst2html"],
        path_tool_binaries = ["//core/quality/tools/python:rst2html"],
    ),
    matrix_case(
        name = "matrix_vale_rst_lint_multi",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "rst",
        generated = {
            "matrix/rst_a.rst": "First\n=====\n\nAll clean here.\n",
            "matrix/rst_b.rst": "Second\n======\n\nSection\n-------\n\n* item one\n* item FIXME two\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["rst2html"],
        path_tool_binaries = ["//core/quality/tools/python:rst2html"],
    ),
    matrix_case(
        name = "matrix_vale_rst_lint_no_converter",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "rst",
        generated = {
            "matrix/rst_nc.rst": "Title\n=====\n\nA clean paragraph.\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        runner_failure = True,
    ),
    matrix_case(
        name = "matrix_vale_asciidoc_lint_pass",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "asciidoc",
        generated = {
            "matrix/asciidoc_clean.adoc": "= Title\n\nA clean paragraph with æøå.\n",
        },
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["asciidoctor"],
        path_tool_binaries = ["//javascript/private/quality/bin:asciidoctor"],
    ),
    matrix_case(
        name = "matrix_vale_asciidoc_lint_fail",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "asciidoc",
        generated = {
            "matrix/asciidoc_dirty.adoc": "= Title\n\nSecond line.\nFix this TODO before merging.\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["asciidoctor"],
        path_tool_binaries = ["//javascript/private/quality/bin:asciidoctor"],
    ),
    matrix_case(
        name = "matrix_vale_asciidoc_lint_multi",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "asciidoc",
        generated = {
            "matrix/asciidoc_a.adoc": "= First\n\nAll clean here.\n",
            "matrix/asciidoc_b.adoc": "= Second\n\n== Section\n\n* item one\n* item FIXME two\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["asciidoctor"],
        path_tool_binaries = ["//javascript/private/quality/bin:asciidoctor"],
    ),
    matrix_case(
        name = "matrix_vale_asciidoc_lint_code_block",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "asciidoc",
        generated = {
            "matrix/asciidoc_code.adoc": "= Title\n\nA clean paragraph.\n\n----\nTODO inside a listing block\n----\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        path_tool_owners = ["vale"],
        path_tool_names = ["asciidoctor"],
        path_tool_binaries = ["//javascript/private/quality/bin:asciidoctor"],
    ),
    matrix_case(
        name = "matrix_vale_asciidoc_lint_no_converter",
        capability = "lint",
        tool = "vale",
        binary = "@dx_tools//:vale",
        source_class = "asciidoc",
        generated = {
            "matrix/asciidoc_nc.adoc": "= Title\n\nA clean paragraph.\n",
        },
        platforms = ["linux_x86_64"],
        toolfile_tools = ["vale", "vale"],
        toolfile_srcs = [":vale_test.ini", "styles/Dx/Markers.yml"],
        config_tools = ["vale"],
        config_files = [":vale_test.ini"],
        runner_failure = True,
    ),
    _vale_profile_case("matrix_markdown_lint_profile_standard", "standard", "markdown", "matrix/vale_profile_standard.md", "# Profile\n\nThe ball was kicked.\n"),
    _vale_profile_case("matrix_markdown_lint_profile_strict", "strict", "markdown", "matrix/vale_profile_strict.md", "# Profile\n\nThe ball was kicked.\n"),
    _clean_tier_case("standard"),
    _clean_tier_case("strict"),
    _vale_profile_case("matrix_rst_lint_profile_strict", "strict", "rst", "matrix/vale_profile_strict.rst", "Profile\n=======\n\nThe ball was kicked.\n"),
]
