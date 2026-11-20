"""Public Elixir repository rules."""

load("//elixir/private:hex.bzl", _hex_package = "hex_package")

# Loaded from consumer MODULE.bazel files through use_repo_rule.
visibility("public")

hex_package = _hex_package
