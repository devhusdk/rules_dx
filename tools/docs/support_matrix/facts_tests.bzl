"""Schema tests for the support matrix facts."""

load("//cc/toolchains:pins.bzl", "GCC_VERSION", "LLVM_CLANG_VERSION", "MSVC_CL_BUILD")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//quality:adapters.bzl", "NOT_APPLICABLE")
load("//quality:support.bzl", "SUPPORT_PLATFORMS")
load("//tools/docs/components:defs.bzl", "astro_doc_shard", "mdx_doc_shard", "svelte_doc_shard", "vue_doc_shard")
load("//tools/docs/dotnet:defs.bzl", "csharp_doc_shard", "fsharp_doc_shard")
load("//tools/docs/go:defs.bzl", "go_doc_shard")
load("//tools/docs/jvm:defs.bzl", "java_doc_shard", "kotlin_doc_shard", "scala_doc_shard")
load("//tools/docs/powershell:defs.bzl", "powershell_doc_shard")
load("//tools/docs/python:defs.bzl", "python_doc_shard")
load("//tools/docs/ruby:defs.bzl", "ruby_doc_shard")
load("//tools/docs/site:site.bzl", "docs_site")
load("//tools/docs/typescript:defs.bzl", "typescript_doc_shard")
load(":facts.bzl", "compiler_blocker_problems", "support_matrix_facts_dict", "tool_not_applicable")
load(":languages.bzl", "DEPENDENCIES", "DOC_TOOLS", "PLATFORMS", "UNSUPPORTED_CONSTRAINTS")

# Loading each symbol proves the shard rule exists; the names are what DOC_TOOLS may point at.
_SHARD_RULES = {
    "astro_doc_shard": astro_doc_shard,
    "csharp_doc_shard": csharp_doc_shard,
    "fsharp_doc_shard": fsharp_doc_shard,
    "go_doc_shard": go_doc_shard,
    "java_doc_shard": java_doc_shard,
    "kotlin_doc_shard": kotlin_doc_shard,
    "mdx_doc_shard": mdx_doc_shard,
    "powershell_doc_shard": powershell_doc_shard,
    "python_doc_shard": python_doc_shard,
    "ruby_doc_shard": ruby_doc_shard,
    "docs_site": docs_site,
    "scala_doc_shard": scala_doc_shard,
    "svelte_doc_shard": svelte_doc_shard,
    "typescript_doc_shard": typescript_doc_shard,
    "vue_doc_shard": vue_doc_shard,
}

def _doc_problems(facts):
    problems = []
    for row in facts["rows"]:
        if row["section"] == "languages" and row["name"] not in DOC_TOOLS:
            problems.append("missing " + row["name"])
    row_names = [row["name"] for row in facts["rows"]]
    for name, entry in DOC_TOOLS.items():
        if name not in row_names:
            problems.append("unknown row " + name)
        elif entry and entry["shard"] not in _SHARD_RULES:
            problems.append(name + " names missing shard " + entry["shard"])
    return problems

def _dependency_problems(facts):
    languages = [row["name"] for row in facts["rows"] if row["section"] == "languages"]
    problems = ["missing " + name for name in languages if name not in DEPENDENCIES]
    return problems + ["unknown row " + name for name in DEPENDENCIES if name not in languages]

def _families_resolve(facts):
    return [
        row["name"] + ":" + family
        for row in facts["rows"]
        for family in row["families"]
        if family not in facts["quality"]
    ]

def _not_applicable_problems(facts):
    return [
        capability + ":" + family
        for capability, entries in facts["not_applicable"].items()
        for family in entries
        if family not in facts["quality"] or facts["quality"][family].get(capability, []) != []
    ]

# Rows with a stated reason that do not yet declare an unsupported constraint_value; shrink as #6175 lands them.
_PENDING_UNSUPPORTED = ["appimage", "apple", "cc_gcc", "cc_msvc", "cuda", "fortran", "ocaml", "perl"]

def support_matrix_facts_tests(name):
    """Declares the support matrix facts schema test."""
    facts = support_matrix_facts_dict()
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "platform order is fixed",
                [platform["id"] for platform in facts["platforms"]],
                [platform["id"] for platform in PLATFORMS],
            ),
            expect_equal(
                "support platform vocabulary matches the matrix platforms",
                SUPPORT_PLATFORMS,
                [platform["id"] for platform in PLATFORMS],
            ),
            expect_equal("every language has a doc tool or a gap", _doc_problems(facts), []),
            expect_equal("every language shows its dependency sets or none", _dependency_problems(facts), []),
            expect_equal("every family resolves", _families_resolve(facts), []),
            expect_equal(
                "rows are sorted by name",
                [row["name"] for row in facts["rows"]],
                sorted([row["name"] for row in facts["rows"]]),
            ),
            expect_equal("not applicable families have no tools", _not_applicable_problems(facts), []),
            expect_equal(
                "typecheck n/a families are an explicit snapshot",
                sorted(NOT_APPLICABLE["typecheck"].keys()),
                ["ansible", "api_spec", "asciidoc", "bicep", "cloudformation", "cmake", "dockerfile", "gherkin", "helm", "html_template", "jsonnet", "kubernetes", "latex", "make", "nix", "perl", "powershell", "rst", "sql", "typst", "xml", "yaml"],
            ),
            expect_equal(
                "records merge per family, capability and tool",
                tool_not_applicable([
                    {"family": "f", "capability": "lint", "tool": "a", "unsupported": {"windows_arm64": "x"}},
                    {"family": "f", "capability": "lint", "tool": "a", "unsupported": {"windows_arm64": "x", "windows_x86_64": "y"}},
                    {"family": "f", "capability": "lint", "tool": "b"},
                    {"family": "g", "capability": "typecheck", "tool": "a", "unsupported": {"windows_arm64": "z"}},
                ]),
                {
                    "f": {"lint": {"a": {"windows_arm64": "x", "windows_x86_64": "y"}}},
                    "g": {"typecheck": {"a": {"windows_arm64": "z"}}},
                },
            ),
            expect_equal(
                "compiler rows have no families",
                [row["name"] for row in facts["rows"] if row["section"] == "compilers" and row["families"]],
                [],
            ),
            expect_equal(
                "compiler rows include cc_llvm, cc_gcc and cc_msvc",
                [row["name"] for row in facts["rows"] if row["section"] == "compilers"],
                ["cc_gcc", "cc_llvm", "cc_msvc"],
            ),
            expect_equal(
                "compiler identity",
                {row["name"]: row["identity"] for row in facts["rows"] if row["section"] == "compilers"},
                {
                    "cc_gcc": {"compiler": "gcc", "coverage": "gcov", "driver": "gnu", "platform_probes": {"linux_arm64": "//cc/toolchains:identity_gcc_arm64_native"}, "probe": "//cc/toolchains:identity_gcc", "version": GCC_VERSION},
                    "cc_llvm": {"compiler": "llvm", "coverage": "llvm", "driver": "gnu", "platform_probes": {}, "probe": "//cc/toolchains:identity", "version": LLVM_CLANG_VERSION},
                    "cc_msvc": {"compiler": "msvc", "coverage": "", "driver": "msvc", "platform_probes": {"windows_arm64": "//cc/toolchains:identity_msvc_arm64_native"}, "probe": "//cc/toolchains:identity_msvc", "version": MSVC_CL_BUILD},
                },
            ),
            expect_equal(
                "compiler blockers",
                facts["compiler_blockers"],
                {},
            ),
            expect_equal(
                "invalid compiler blockers are reported",
                compiler_blocker_problems(
                    {
                        "c": {"p": "x", "q": "y", "r": ""},
                        "l": {"p": "x"},
                    },
                    [{"name": "c", "section": "compilers"}, {"name": "l", "section": "languages"}],
                    [{"id": "p"}, {"id": "r"}],
                    {"c": {"p": "never"}},
                ),
                [
                    "c on p is both blocked and not applicable",
                    "c needs a known platform and a reason, got q",
                    "c needs a known platform and a reason, got r",
                    "l is not a compiler row",
                ],
            ),
            expect_equal("pending compilers", sorted(facts["pending_compilers"].keys()), []),
            expect_equal(
                "every row with a stated reason has an unsupported constraint_value or is pending",
                sorted([name for name in facts["platform_not_applicable"] if name not in UNSUPPORTED_CONSTRAINTS]),
                _PENDING_UNSUPPORTED,
            ),
            expect_equal(
                "unsupported constraint labels name the row's unsupported value",
                [name for name, label in facts["unsupported_constraints"].items() if not label.endswith(":unsupported")],
                [],
            ),
            expect_equal(
                "api compat tools name a publishing rule each",
                [rule for rule, tool in facts["api_compat_tools"].items() if not rule.endswith("_deploy") or not tool],
                [],
            ),
            expect_equal(
                "rules without an api compat tool are an explicit snapshot",
                sorted([rule for rule, tool in facts["api_compat_tools"].items() if tool == "not available"]),
                ["hackage_deploy", "hex_deploy", "luarocks_deploy", "pub_deploy", "rubygems_deploy"],
            ),
            expect_true("rows are listed", len(facts["rows"]) > 0),
        ],
    )
