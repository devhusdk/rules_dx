"""Public environment tool API."""

load(
    "//env/private:defs.bzl",
    _EnvironmentInfo = "EnvInfo",
    _environment_config = "env_config",
    _environment_metadata = "env_metadata",
    _environment_tool = "env_tool",
    _environment_tree = "env_tree",
)

EnvInfo = _EnvironmentInfo
env_config = _environment_config
env_metadata = _environment_metadata
env_tool = _environment_tool
env_tree = _environment_tree
