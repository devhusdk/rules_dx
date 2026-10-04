"""Explicit platform-keyed tool repository selection."""

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["os_linux", "cpu_x86_64"],
    "linux_arm64": ["os_linux", "cpu_arm64"],
    "macos_arm64": ["os_macos", "cpu_arm64"],
    "windows_x86_64": ["os_windows", "cpu_x86_64"],
}

TOOL_PLATFORMS = list(PLATFORM_CONSTRAINTS.keys())

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
    return [platform for platform in TOOL_PLATFORMS if platform in entries]

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
    for tool in sorted(artifacts.keys()):
        lines.append(_alias(artifacts[tool], tool))
    return "\n".join(lines)

def _platform_names():
    """Returns the supported platform names as one list."""
    return ", ".join(TOOL_PLATFORMS)

def _config_settings(constraints):
    lines = []
    for platform in TOOL_PLATFORMS:
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
