"""Local-only execution requirements helper with per-tool remote qualification."""

load("//core/quality:adapters.bzl", "REAL_ADAPTERS")
load("//core/quality/artifacts:hub.bzl", "TOOL_PLATFORMS")

visibility("private")

DX_NO_REMOTE_EXEC = "no-remote-exec"

DX_FORBIDDEN_NO_REMOTE = "no-remote"

def _qualified(tool, capability):
    """One remote record for a pinned single-binary tool on every qualified platform."""
    return {
        "capability": capability,
        "closure": "declared",
        "platforms": list(TOOL_PLATFORMS),
        "runner": "hermetic",
        "state": "qualified",
        "tool": tool,
    }

SHFMT_FORMAT_REMOTE = _qualified("shfmt", "format")

# A tool qualifies when its action reads only declared inputs: a pinned @dx_tools
# binary built for the exec platform (cfg = "exec"), the staged sources and configs,
# and the runner's hermetic env (no HOME, no host PATH, no network). A stage runs
# remotely only when every tool in it is listed here, see dx_stage_execution_requirements.
# PATH helpers a tool spawns must be pinned files; a stage that runs a runfiles
# launcher not in REMOTE_QUALIFIED_LAUNCHERS stays local.
REMOTE_QUALIFIED = [
    SHFMT_FORMAT_REMOTE,
    _qualified("actionlint", "lint"),
    _qualified("biome", "format"),
    _qualified("biome", "lint"),
    _qualified("buildifier", "format"),
    _qualified("buildifier", "lint"),
    _qualified("checkmake", "lint"),
    _qualified("gersemi", "format"),
    _qualified("hadolint", "lint"),
    _qualified("jsonnetfmt", "format"),
    _qualified("keep_sorted", "lint"),
    _qualified("ruff", "format"),
    _qualified("ruff", "lint"),
    _qualified("shellcheck", "lint"),
    _qualified("stylua", "format"),
    _qualified("taplo", "format"),
    _qualified("taplo", "lint"),
    _qualified("typos", "lint"),
    _qualified("typstyle", "format"),
    _qualified("vale", "lint"),
    _qualified("yamlfmt", "format"),
    _qualified("zizmor", "lint"),
]

# Tool and capability pairs that must never run remotely, with the reason.
LOCAL_ONLY = {
    ("lychee", "lint"): "DX_LINT_ONLINE=1 fetches every http(s) link, and a remote executor has no network",
}

# Tool and capability pairs not yet checked for remote execution. They stay local.
# Move each one to REMOTE_QUALIFIED or LOCAL_ONLY once it is checked.
PENDING = [
    ("ansible_lint", "lint"),
    ("avro_tools", "lint"),
    ("avro_tools", "typecheck"),
    ("bicep", "format"),
    ("bicep", "lint"),
    ("buf", "format"),
    ("buf", "lint"),
    ("buf", "typecheck"),
    ("capnp", "lint"),
    ("capnp", "typecheck"),
    ("cfn_lint", "lint"),
    ("chktex", "lint"),
    ("clang_format", "format"),
    ("clang_syntax", "typecheck"),
    ("clang_tidy", "lint"),
    ("clippy", "lint"),
    ("cmakelint", "lint"),
    ("container_structure_test", "typecheck"),
    ("credo", "lint"),
    ("csc", "typecheck"),
    ("csharpier", "format"),
    ("cue", "format"),
    ("cue", "typecheck"),
    ("dart", "format"),
    ("dart", "lint"),
    ("dart", "typecheck"),
    ("deadnix", "lint"),
    ("detekt", "lint"),
    ("dialyzer", "typecheck"),
    ("djlint", "format"),
    ("djlint", "lint"),
    ("docstrfmt", "format"),
    ("dscanner", "lint"),
    ("elixirc", "typecheck"),
    ("elvis", "lint"),
    ("erlc", "typecheck"),
    ("erlfmt", "format"),
    ("errcheck", "lint"),
    ("eslint", "lint"),
    ("fantomas", "format"),
    ("ffmt", "format"),
    ("flang", "typecheck"),
    ("flatc", "lint"),
    ("flatc", "typecheck"),
    ("fortitude", "lint"),
    ("fsc", "typecheck"),
    ("fsharplint", "lint"),
    ("ghc", "typecheck"),
    ("gofumpt", "format"),
    ("google_java_format", "format"),
    ("gotype", "typecheck"),
    ("govet", "lint"),
    ("gplint", "lint"),
    ("graphql_eslint", "lint"),
    ("graphql_eslint", "typecheck"),
    ("helm", "lint"),
    ("helmfmt", "format"),
    ("hlint", "lint"),
    ("javac", "typecheck"),
    ("jet", "typecheck"),
    ("jsonnet_lint", "lint"),
    ("julia_format", "format"),
    ("julialint", "lint"),
    ("kotlinc", "typecheck"),
    ("ktfmt", "format"),
    ("kubeconform", "lint"),
    ("latexindent", "format"),
    ("ldc", "typecheck"),
    ("luals", "typecheck"),
    ("markdown_check", "lint"),
    ("mbake", "format"),
    ("mix_format", "format"),
    ("modfmt", "format"),
    ("nixfmt", "format"),
    ("notebook_outputs", "lint"),
    ("nvcc_syntax", "typecheck"),
    ("ocaml", "typecheck"),
    ("opa", "format"),
    ("opa", "typecheck"),
    ("ormolu", "format"),
    ("perlcritic", "lint"),
    ("perltidy", "format"),
    ("phpcs", "lint"),
    ("phpstan", "typecheck"),
    ("pkl", "format"),
    ("pkl", "typecheck"),
    ("pkl_lsp", "lint"),
    ("pmd", "lint"),
    ("prettier", "format"),
    ("psscriptanalyzer", "format"),
    ("psscriptanalyzer", "lint"),
    ("pydoclint", "lint"),
    ("qmlformat", "format"),
    ("qmllint", "lint"),
    ("qmllint", "typecheck"),
    ("redocly", "lint"),
    ("regal", "lint"),
    ("remark", "format"),
    ("roslyn", "lint"),
    ("rubocop", "format"),
    ("rubocop", "lint"),
    ("ruff", "audit"),
    ("rustc", "typecheck"),
    ("rustfmt", "format"),
    ("scalac", "typecheck"),
    ("scalafix", "lint"),
    ("scalafmt", "format"),
    ("selene", "lint"),
    ("sorbet", "typecheck"),
    ("spotbugs", "lint"),
    ("sqlfluff", "format"),
    ("sqlfluff", "lint"),
    ("staticcheck", "lint"),
    ("stylelint", "lint"),
    ("swift_format", "format"),
    ("swift_lint", "lint"),
    ("swiftc", "typecheck"),
    ("taplo", "typecheck"),
    ("terraform", "format"),
    ("terraform", "typecheck"),
    ("tflint", "lint"),
    ("thrift", "lint"),
    ("thrift", "typecheck"),
    ("tinymist", "lint"),
    ("topiary", "format"),
    ("trivy_config", "lint"),
    ("trivy_image_config", "lint"),
    ("tsc", "typecheck"),
    ("ty", "typecheck"),
    ("weeder", "lint"),
    ("xmllint", "lint"),
    ("yamllint", "lint"),
    ("zanuda", "lint"),
    ("zig", "typecheck"),
    ("zig_ast_check", "lint"),
    ("zig_fmt", "format"),
]

def classification_errors(real_adapters, qualified, local_only, pending):
    """Lists every adapter pair missing from, or repeated across, the qualified, local-only and pending lists."""
    seen = {}
    errors = []
    for name, pairs in [
        ("REMOTE_QUALIFIED", [(r["tool"], r["capability"]) for r in qualified]),
        ("LOCAL_ONLY", list(local_only.keys())),
        ("PENDING", pending),
    ]:
        for pair in pairs:
            label = pair[0] + " " + pair[1]
            if pair[0] not in real_adapters or pair[1] not in real_adapters[pair[0]]:
                errors.append("remote: " + name + " lists '" + label + "', which is not a real adapter")
            elif pair in seen:
                errors.append("remote: '" + label + "' is in both " + seen[pair] + " and " + name)
            else:
                seen[pair] = name
    for pair, reason in local_only.items():
        if reason.strip() == "":
            errors.append("remote: LOCAL_ONLY '" + pair[0] + " " + pair[1] + "' needs a reason")
    for tool in sorted(real_adapters.keys()):
        for capability in sorted(real_adapters[tool].keys()):
            if (tool, capability) not in seen:
                errors.append("remote: '" + tool + " " + capability + "' is in none of REMOTE_QUALIFIED, LOCAL_ONLY or PENDING")
    return errors

def dx_is_local_only(requirements):
    """Reports whether requirements carry the local-only marker."""
    return requirements.get(DX_NO_REMOTE_EXEC, "") == "1"

def dx_has_forbidden_marker(requirements):
    """Reports whether requirements carry the cache-disabling marker."""
    return DX_FORBIDDEN_NO_REMOTE in requirements

def remote_record_error(record):
    """Validates one remote qualification record against the adapter taxonomy."""
    for key in ["capability", "closure", "platforms", "runner", "state", "tool"]:
        if key not in record:
            return "remote: record is missing field '" + key + "'"
    tool = record["tool"]
    capability = record["capability"]
    if tool not in REAL_ADAPTERS:
        return "remote: unknown tool '" + tool + "': not in the real adapter registry"
    if capability not in REAL_ADAPTERS[tool]:
        return "remote: tool '" + tool + "' has no '" + capability + "' capability"
    if record["closure"] != "declared":
        return "remote: tool '" + tool + "' needs closure 'declared', got '" + str(record["closure"]) + "'"
    if record["runner"] != "hermetic":
        return "remote: tool '" + tool + "' needs runner 'hermetic', got '" + str(record["runner"]) + "'"
    if record["state"] != "qualified":
        return "remote: tool '" + tool + "' needs state 'qualified', got '" + str(record["state"]) + "'"
    platforms = record["platforms"]
    if type(platforms) != "list" or len(platforms) == 0:
        return "remote: tool '" + tool + "' needs a non-empty execution platform list"
    for platform in platforms:
        if platform not in TOOL_PLATFORMS:
            return "remote: tool '" + tool + "' names unknown execution platform '" + str(platform) + "'"
    return ""

def remote_platform_error(record, artifact_platforms):
    """Reports the platforms a remote record claims without a pinned artifact."""
    covered = artifact_platforms.get(record["tool"], [])
    missing = [platform for platform in record["platforms"] if platform not in covered]
    if len(missing) > 0:
        return "remote: tool '" + record["tool"] + "' is remote-qualified for " + ", ".join(missing) + " without a pinned artifact"
    return ""

def dx_remote_platforms(tool, capability):
    """Returns the qualified execution platforms for one tool and capability."""
    for record in REMOTE_QUALIFIED:
        if record["tool"] == tool and record["capability"] == capability:
            return list(record["platforms"])
    return []

def dx_is_remote_qualified(tool, capability):
    """Reports whether one tool and capability carry a valid remote record."""
    for record in REMOTE_QUALIFIED:
        if record["tool"] == tool and record["capability"] == capability:
            return remote_record_error(record) == ""
    return False

# Runfiles launchers a remote stage may run. The runner hands each one its own
# staged runfiles tree.
REMOTE_QUALIFIED_LAUNCHERS = [
    # js_binary: reads `<binary>.runfiles` or the manifest beside it.
    "asciidoctor",
    # py_binary: the runner sets RUNFILES_DIR when no manifest is staged.
    "rst2html",
]

def dx_stage_execution_requirements(stage_tools, capability, run_tools = []):
    """Returns remote-capable requirements only when every stage and every runfiles launcher is qualified.

    Args:
      stage_tools: tool ids the stage runs.
      capability: "format" or "lint".
      run_tools: names of the runfiles launchers the stage puts on PATH.
    """
    if len(stage_tools) == 0:
        return {DX_NO_REMOTE_EXEC: "1"}
    for name in run_tools:
        if name not in REMOTE_QUALIFIED_LAUNCHERS:
            return {DX_NO_REMOTE_EXEC: "1"}
    for tool in stage_tools:
        if not dx_is_remote_qualified(tool, capability):
            return {DX_NO_REMOTE_EXEC: "1"}
    return {}
