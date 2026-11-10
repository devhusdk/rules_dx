"""Public environment tool API."""

load(
    "//env/private:defs.bzl",
    _EnvironmentInfo = "EnvironmentInfo",
    _env_host_filename = "env_host_filename",
    _env_tool_record = "env_tool_record",
    _env_tree_metadata = "env_tree_metadata",
    _environment_config = "environment_config",
    _environment_metadata = "environment_metadata",
    _environment_tool = "environment_tool",
    _environment_tree = "environment_tree",
)

EnvironmentInfo = _EnvironmentInfo
env_host_filename = _env_host_filename
env_tool_record = _env_tool_record
env_tree_metadata = _env_tree_metadata
environment_config = _environment_config
environment_metadata = _environment_metadata
environment_tool = _environment_tool
environment_tree = _environment_tree
