"""Public Elixir repository rules."""

load("//elixir/private:hex.bzl", _hex_package = "hex_package")

visibility("public")

hex_package = _hex_package
