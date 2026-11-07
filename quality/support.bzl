"""Explicit tool support records."""

load("//quality/artifacts:extension.bzl", "TOOL_ARTIFACTS")
load("//quality/artifacts:hub.bzl", "EMULATED_TOOLS", "PLATFORM_FALLBACKS", "TOOL_PLATFORMS")
load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load(":tools.bzl", "TOOLS", "is_canonical_token")

SUPPORT_DEFAULTS = {
    "artifact": "pinned",
    "execution": "direct",
}

def support(**overrides):
    """Returns one support record with the shared defaults filled in."""
    return dict(SUPPORT_DEFAULTS, **overrides)

_RAW_RECORDS = [support(**record) for tool in TOOLS for record in tool.support]

def _record(family, tool, capability):
    for record in _RAW_RECORDS:
        if (record["family"], record["tool"], record["capability"]) == (family, tool, capability):
            return record
    fail("support: no record for %s %s %s" % (family, tool, capability))

ANSIBLE_YAMLFMT_FORMAT_SUPPORT = _record("ansible", "yamlfmt", "format")
API_SPEC_PRETTIER_FORMAT_SUPPORT = _record("api_spec", "prettier", "format")
API_SPEC_REDOCLY_LINT_SUPPORT = _record("api_spec", "redocly", "lint")
BICEP_BICEP_FORMAT_SUPPORT = _record("bicep", "bicep", "format")
BICEP_BICEP_LINT_SUPPORT = _record("bicep", "bicep", "lint")
CC_CLANG_FORMAT_SUPPORT = _record("cc", "clang_format", "format")
CC_CPPCHECK_LINT_SUPPORT = _record("cc", "cppcheck", "lint")
CLOUDFORMATION_CFN_LINT_SUPPORT = _record("cloudformation", "cfn_lint", "lint")
CMAKE_CMAKELINT_SUPPORT = _record("cmake", "cmakelint", "lint")
CUDA_CLANG_FORMAT_SUPPORT = _record("cuda", "clang_format", "format")
DOCKERFILE_PRETTIER_FORMAT_SUPPORT = _record("dockerfile", "prettier", "format")
GHERKIN_GPLINT_LINT_SUPPORT = _record("gherkin", "gplint", "lint")
GHERKIN_PRETTIER_FORMAT_SUPPORT = _record("gherkin", "prettier", "format")
JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT = _record("jupyter", "notebook_outputs", "lint")
JUPYTER_RUFF_FORMAT_SUPPORT = _record("jupyter", "ruff", "format")
JUPYTER_RUFF_LINT_SUPPORT = _record("jupyter", "ruff", "lint")
KUBERNETES_YAMLFMT_FORMAT_SUPPORT = _record("kubernetes", "yamlfmt", "format")
MAKE_MBAKE_SUPPORT = _record("make", "mbake", "format")
OBJC_CLANG_FORMAT_SUPPORT = _record("objc", "clang_format", "format")
PHP_PRETTIER_FORMAT_SUPPORT = _record("php", "prettier", "format")
PYTHON_PYDOCLINT_LINT_SUPPORT = _record("python", "pydoclint", "lint")
PYTHON_RUFF_FORMAT_SUPPORT = _record("python", "ruff", "format")
PYTHON_RUFF_LINT_SUPPORT = _record("python", "ruff", "lint")
PYTHON_TY_TYPECHECK_SUPPORT = _record("python", "ty", "typecheck")
RUST_RUSTFMT_FORMAT_SUPPORT = _record("rust", "rustfmt", "format")
SQL_SQLFLUFF_FORMAT_SUPPORT = _record("sql", "sqlfluff", "format")
SQL_SQLFLUFF_LINT_SUPPORT = _record("sql", "sqlfluff", "lint")
TERRAFORM_FORMAT_SUPPORT = _record("terraform", "terraform", "format")
TERRAFORM_TYPECHECK_SUPPORT = _record("terraform", "terraform", "typecheck")
TEXT_KEEP_SORTED_SUPPORT = _record("text", "keep_sorted", "lint")
XML_PRETTIER_FORMAT_SUPPORT = _record("xml", "prettier", "format")

SUPPORT_PLATFORMS = TOOL_PLATFORMS + sorted(PLATFORM_FALLBACKS.keys())

def _pinned_platforms():
    """Maps each tool to the platforms of its pinned artifacts."""
    pinned = {}
    for artifact in TOOL_ARTIFACTS:
        pinned.setdefault(artifact["tool"], []).append(artifact["os"] + "_" + artifact["cpu"])
    return pinned

_PINNED_PLATFORMS = _pinned_platforms()

def with_platforms(record):
    """Fills in the platforms of a record from the tool artifact table."""
    if "platforms" in record:
        return record
    tool = record["tool"]
    pinned = _PINNED_PLATFORMS.get(tool, [])
    platforms = [p for p in pinned if p in SUPPORT_PLATFORMS]
    for platform, source in PLATFORM_FALLBACKS.items():
        if source in pinned and platform not in platforms and tool in EMULATED_TOOLS.get(platform, []):
            platforms.append(platform)
    unsupported = record.get("unsupported", {}).keys()
    return dict(record, platforms = sorted([p for p in platforms if p not in unsupported]))

SUPPORT_RECORDS = [with_platforms(record) for record in _RAW_RECORDS]

def _is_unbuilt(record):
    """Reports whether a record names a tool with no pinned build on any platform.

    Only a known record with a missing artifact may leave `platforms` empty, and it must list
    every platform as unsupported.
    """
    unsupported = record.get("unsupported", {})
    return (
        record["artifact"] == "missing" and
        record["state"] == "known" and
        type(unsupported) == "dict" and
        sorted(unsupported.keys()) == sorted(SUPPORT_PLATFORMS)
    )

def support_unpinned_tools(capability, records = None):
    """Maps each tool of one capability with no pinned build anywhere to its unsupported platforms."""
    unpinned = {}
    for record in (SUPPORT_RECORDS if records == None else records):
        if record["capability"] == capability and len(record["platforms"]) == 0 and _is_unbuilt(record):
            unpinned[record["tool"]] = sorted(record["unsupported"].keys())
    return unpinned

def support_unsupported_reasons(tools, records = None):
    """Maps each tool to its unsupported-platform reasons joined into one line."""
    reasons = {}
    for record in (SUPPORT_RECORDS if records == None else records):
        if record["tool"] in tools and record.get("unsupported"):
            parts = [platform + ": " + why for platform, why in sorted(record["unsupported"].items())]
            reasons.setdefault(record["tool"], "; ".join(parts))
    return reasons

def support_record_error(record):
    """Validates one support record against the adapter taxonomy."""
    for key in ["artifact", "capability", "classes", "context", "execution", "family", "fix", "platforms", "state", "tool", "transport"]:
        if key not in record:
            return "support: record is missing field '" + key + "'"
    tool = record["tool"]
    capability = record["capability"]
    family = record["family"]
    if not is_canonical_token(tool):
        return "support: non-canonical tool '" + str(tool) + "'"
    if capability not in ["audit", "format", "lint", "typecheck"]:
        return "support: unknown capability '" + str(capability) + "' for tool '" + tool + "'"
    if not is_canonical_token(family):
        return "support: non-canonical family '" + str(family) + "'"
    if tool not in REAL_ADAPTERS:
        return "support: unknown tool '" + tool + "': not in the real adapter registry"
    if capability not in REAL_ADAPTERS[tool]:
        return "support: tool '" + tool + "' has no '" + capability + "' capability"
    classes = record["classes"]
    if type(classes) != "list" or len(classes) == 0:
        return "support: tool '" + tool + "' needs a non-empty class list"
    for class_id in classes:
        if not is_canonical_token(class_id):
            return "support: non-canonical class '" + str(class_id) + "' for tool '" + tool + "'"
        if class_id not in REAL_CLASS_TO_FAMILY:
            return "support: tool '" + tool + "' names unclassified class '" + class_id + "'"
        if REAL_CLASS_TO_FAMILY[class_id] != family:
            return "support: class '" + class_id + "' belongs to family '" + REAL_CLASS_TO_FAMILY[class_id] + "', not '" + family + "'"
        if class_id not in REAL_ADAPTERS[tool][capability]:
            return "support: class '" + class_id + "' is outside '" + tool + " " + capability + "' adapter support"
    if record["execution"] not in ["delegated", "direct"]:
        return "support: tool '" + tool + "' needs execution 'direct' or 'delegated', got '" + str(record["execution"]) + "'"
    if record["fix"] not in ["check_only", "supported"]:
        return "support: tool '" + tool + "' needs fix 'supported' or 'check_only', got '" + str(record["fix"]) + "'"
    if record["context"] not in ["owned", "target_coupled"]:
        return "support: tool '" + tool + "' needs context 'owned' or 'target_coupled', got '" + str(record["context"]) + "'"
    platforms = record["platforms"]
    if type(platforms) != "list":
        return "support: tool '" + tool + "' needs a non-empty platform list"
    if len(platforms) == 0 and not _is_unbuilt(record):
        return "support: tool '" + tool + "' needs a non-empty platform list"
    for platform in platforms:
        if not is_canonical_token(platform):
            return "support: non-canonical platform '" + str(platform) + "' for tool '" + tool + "'"
        if platform not in SUPPORT_PLATFORMS:
            return "support: unknown platform '" + str(platform) + "' for tool '" + tool + "'"
    unsupported = record.get("unsupported", {})
    if type(unsupported) != "dict":
        return "support: tool '" + tool + "' needs 'unsupported' to map each platform to a one-line reason"
    for platform, reason in unsupported.items():
        if platform not in SUPPORT_PLATFORMS:
            return "support: unknown unsupported platform '" + str(platform) + "' for tool '" + tool + "'"
        if platform in platforms:
            return "support: platform '" + platform + "' is both supported and unsupported for tool '" + tool + "'"
        if type(reason) != "string" or reason == "" or "\n" in reason:
            return "support: tool '" + tool + "' needs 'unsupported' to map each platform to a one-line reason"
    if record["state"] not in ["known", "qualified", "supported"]:
        return "support: tool '" + tool + "' needs state 'known', 'supported' or 'qualified', got '" + str(record["state"]) + "'"
    if record["artifact"] not in ["missing", "pinned"]:
        return "support: tool '" + tool + "' needs artifact 'missing' or 'pinned', got '" + str(record["artifact"]) + "'"
    if record["transport"] not in ["batched", "single"]:
        return "support: tool '" + tool + "' needs transport 'batched' or 'single', got '" + str(record["transport"]) + "'"
    return ""

def support_schema_error():
    """Validates every recorded support entry."""
    for record in SUPPORT_RECORDS:
        err = support_record_error(record)
        if err != "":
            return err
    return ""

def support_record_gap_error(record, wired):
    """Reports the first wiring or pin gap for one supported or qualified record."""
    state = record["state"]
    if state not in ["qualified", "supported"]:
        return ""
    prefix = "support: " + record["tool"] + " " + record["capability"] + " is " + state
    if record["capability"] not in wired.get(record["tool"], []):
        return prefix + " but has no wired aspect (not silently clean)"
    if record["artifact"] != "pinned":
        return prefix + " but has no pinned artifact"
    return ""

def support_gap_error(wired):
    """Returns the first wiring or pin gap across every support record."""
    for record in SUPPORT_RECORDS:
        err = support_record_gap_error(record, wired)
        if err != "":
            return err
    return ""

def is_support_tool(tool_id):
    """Reports whether a tool carries an explicit support record."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return True
    return False

def support_state(tool_id):
    """Returns the recorded support state for one tool, or unknown."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return record["state"]
    return "unknown"
