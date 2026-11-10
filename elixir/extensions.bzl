"""Public Elixir module extension and repository rules."""

load("//elixir/private:hex.bzl", _hex_package = "hex_package")
load("//elixir/private/toolchain:extensions.bzl", _elixir = "elixir")

elixir = _elixir
hex_package = _hex_package
