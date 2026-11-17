"""Placeholder until bazel run //core/quality:tools_registry renders the registry."""

load("//tools/quality:family_tools.bzl", _FAMILY_TOOLS = "FAMILY_TOOLS")

FAMILY_TOOL_ROWS = json.decode(json.encode(_FAMILY_TOOLS))
