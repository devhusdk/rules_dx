"""Explicit platform-keyed tool repository selection."""

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["os_linux", "cpu_x86_64"],
    "linux_arm64": ["os_linux", "cpu_arm64"],
    "macos_arm64": ["os_macos", "cpu_arm64"],
    "windows_x86_64": ["os_windows", "cpu_x86_64"],
    "macos_x86_64": ["os_macos", "cpu_x86_64"],
    "windows_arm64": ["os_windows", "cpu_arm64"],
}

# The qualified execution set: every dx tool ships for each of these, and
# signing, remote execution and is_supported_key read it. Keep it explicit so a
# selectable platform added to PLATFORM_CONSTRAINTS does not qualify itself.
TOOL_PLATFORMS = [
    "linux_x86_64",
    "linux_arm64",
    "macos_arm64",
    "windows_x86_64",
]

# Selectable platforms without a native build resolve to a qualified platform's
# artifact, run under OS emulation (Windows on Arm runs x64 via Prism). Keys must
# be outside TOOL_PLATFORMS and values inside it: qualified platforms never run
# emulated binaries. A native artifact on the key platform always wins.
# Fallback resolution supplies an artifact, never a support claim: a record claims
# the key platform only with a matrix case flagged for it.
PLATFORM_FALLBACKS = {
    "windows_arm64": "windows_x86_64",
}

# Inventory of tools whose upstream release ships no native build for a
# fallback platform, so they run emulated there. Inventory only: resolution
# stays in PLATFORM_FALLBACKS. A test requires every tool with a pin on the
# fallback target to be either natively pinned on the key platform or listed
# here (sorted), so a new upstream build forces this list to shrink.
EMULATED_TOOLS = {
    "windows_arm64": [
        "ffmt",
        "hadolint",
        "hlint",
        "keep_sorted",
        "latexindent",
        "mdbook",
        "opa",
        "ormolu",
        "pkl",
        "scalafmt",
        "shellcheck",
        "shfmt",
        "stylua",
        "tflint",
    ],
}

# Tools pinned on only some qualified platforms (upstream ships no build for the
# rest). The hub renders a default branch to an empty file set for them instead
# of failing analysis, so unrelated builds on other platforms keep working;
# consumers that need the tool fail with a hint to set a tool override.
PARTIAL_TOOL_PLATFORMS = {
    "hlint": ["linux_x86_64", "windows_x86_64"],
    "nixfmt": ["linux_x86_64"],
    "ormolu": ["linux_x86_64", "windows_x86_64"],
}

# Name of the empty target a partial tool resolves to on an unpinned platform.
UNAVAILABLE_TARGET = "dx_unavailable"

# Every platform the hub renders a config_setting for and accepts artifacts on.
# Platforms outside TOOL_PLATFORMS are selectable but unqualified.
ARTIFACT_PLATFORMS = list(PLATFORM_CONSTRAINTS.keys())

_ARTIFACT_KEYS = [
    "archive",
    "cpu",
    "executable",
    "executable_sha256",
    "os",
    "sha256",
    "tool",
    "url",
]

_MESSAGE_COLUMNS = 68

def encode_artifacts(by_tool):
    """Encodes one platform-keyed repository map as deterministic JSON."""
    return json.encode(by_tool)

def decode_artifacts(raw):
    """Decodes one JSON repository map into artifacts and a diagnostic."""
    artifacts = json.decode(raw)
    if type(artifacts) != "dict":
        return struct(artifacts = {}, error = "artifacts must be a JSON object keyed by tool, got " + type(artifacts))
    return struct(artifacts = artifacts, error = "")

def ordered_platforms(entries):
    """Returns the platform keys one tool map holds, in platform order."""
    return [platform for platform in ARTIFACT_PLATFORMS if platform in entries]

def resolved_entries(entries):
    """Returns one tool map plus fallback platforms it has no native artifact for."""
    resolved = dict(entries)
    for platform, source in PLATFORM_FALLBACKS.items():
        if platform not in resolved and source in entries:
            resolved[platform] = entries[source]
    return resolved

def artifact_metadata_errors(artifacts):
    """Returns one error string per artifact with missing or duplicated metadata."""
    errors = []
    declared = {}
    for artifact in artifacts:
        if type(artifact) != "dict":
            errors.append("artifact is a " + type(artifact) + ", want an object of artifact metadata")
            continue
        tool = str(artifact.get("tool", ""))
        missing = [key for key in _ARTIFACT_KEYS if artifact.get(key, "") == ""]
        if "archive" not in missing and (
            type(artifact["archive"]) != "dict" or
            artifact["archive"].get("format", "") == ""
        ):
            missing.append("archive.format")
        if len(missing) > 0:
            errors.append("artifact '" + tool + "' is missing " + ", ".join(missing))
        if artifact.get("os", "") == "" or artifact.get("cpu", "") == "":
            continue
        platform = str(artifact["os"]) + "_" + str(artifact["cpu"])
        if platform not in PLATFORM_CONSTRAINTS:
            errors.append("artifact '" + tool + "' names unsupported platform '" + platform +
                          "'; want one of " + _platform_names())
        entry = tool + "/" + platform
        if entry in declared:
            errors.append("artifact '" + entry + "' is declared twice: '" + declared[entry] +
                          "' and '" + str(artifact.get("url", "")) + "'")
        else:
            declared[entry] = str(artifact.get("url", ""))
    return errors

def artifact_map_errors(artifacts):
    """Returns one error string per invalid tool-to-platform repository map."""
    errors = []
    selected = {}
    for tool in sorted(artifacts.keys()):
        entries = artifacts[tool]
        if tool == "":
            errors.append("one tool name is empty")
        if type(entries) != "dict":
            errors.append("tool '" + tool + "' maps platforms to a " + type(entries) +
                          ", want an object keyed by platform")
            continue
        if len(entries) == 0:
            errors.append("tool '" + tool + "' has no artifact for any platform")
        for platform in sorted(entries.keys()):
            repo = entries[platform]
            if platform not in PLATFORM_CONSTRAINTS:
                errors.append("tool '" + tool + "' names unsupported platform '" + platform +
                              "'; want one of " + _platform_names())
            if type(repo) != "string" or repo == "":
                errors.append("tool '" + tool + "' platform '" + platform + "' has no repository name")
                continue
            selection = tool + "/" + platform
            if repo in selected:
                errors.append("repository '" + repo + "' is selected by both '" + selected[repo] +
                              "' and '" + selection + "'")
            else:
                selected[repo] = selection
    return errors

def branch_labels(entries):
    """Returns the platform-keyed tool repository labels for one tool map."""
    return {":" + platform: "@" + entries[platform] + "//:tool" for platform in ordered_platforms(entries)}

def no_match_error(entries, tool):
    """Returns the diagnostic for a tool without an artifact for the execution platform."""
    return ("rules_dx: no " + tool + " artifact for this execution platform; want one of " +
            ", ".join(ordered_platforms(entries)) + ".")

def hub_build(artifacts, constraints):
    """Renders the hub BUILD file for one tool-to-platform repository map."""
    lines = [_config_settings(constraints)]
    if [tool for tool in artifacts if tool in PARTIAL_TOOL_PLATFORMS]:
        lines.append('filegroup(\n    name = "%s",\n    srcs = [],\n)\n' % UNAVAILABLE_TARGET)
    for tool in sorted(artifacts.keys()):
        lines.append(_alias(resolved_entries(artifacts[tool]), tool))
    return "\n".join(lines)

def _platform_names():
    """Returns the selectable platform names as one list."""
    return ", ".join(ARTIFACT_PLATFORMS)

def _config_settings(constraints):
    lines = []
    for platform in ARTIFACT_PLATFORMS:
        lines.append("config_setting(")
        lines.append('    name = "%s",' % platform)
        lines.append("    constraint_values = [")
        for attr in PLATFORM_CONSTRAINTS[platform]:
            lines.append('        "%s",' % constraints[attr])
        lines.append("    ],")
        lines.append(")")
    return "\n".join(lines) + "\n"

def _alias(entries, tool):
    lines = ["alias("]
    lines.append('    name = "%s",' % tool)
    lines.append("    actual = select(")
    lines.append("        {")
    for platform in ordered_platforms(entries):
        lines.append('            ":%s": "@%s//:tool",' % (platform, entries[platform]))
    if tool in PARTIAL_TOOL_PLATFORMS:
        lines.append('            "//conditions:default": ":%s",' % UNAVAILABLE_TARGET)
        lines.append("        },")
    else:
        lines.append("        },")
        lines.append("        no_match_error = (")
        lines.extend(_message_lines("            ", no_match_error(entries, tool)))
        lines.append("        ),")
    lines.append("    ),")
    lines.append('    visibility = ["//visibility:public"],')
    lines.append(")")
    return "\n".join(lines) + "\n"

def _message_lines(indent, message):
    words = message.split(" ")
    parts = []
    chunk = words[0]
    for word in words[1:]:
        if len(chunk) + 1 + len(word) <= _MESSAGE_COLUMNS:
            chunk += " " + word
        else:
            parts.append(chunk)
            chunk = word
    parts.append(chunk)
    lines = []
    for part in parts[:-1]:
        lines.append(indent + '"' + part + ' " +')
    lines.append(indent + '"' + parts[-1] + '"')
    return lines
