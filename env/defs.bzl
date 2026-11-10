"""Public environment tool API."""

load(
    "//env/private:defs.bzl",
    _EnvironmentInfo = "EnvironmentInfo",
    _environment_config = "environment_config",
    _environment_metadata = "environment_metadata",
    _environment_tool = "environment_tool",
    _environment_tree = "environment_tree",
)

EnvironmentInfo = _EnvironmentInfo
environment_config = _environment_config
environment_metadata = _environment_metadata
environment_tool = _environment_tool
environment_tree = _environment_tree
