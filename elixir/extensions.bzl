"""Public Elixir module extensions and repository rules."""

load("//elixir/private:hex.bzl", _hex_package = "hex_package")
load("//elixir/private/lock:hub.bzl", _hex = "hex")
load("//elixir/private/toolchain:extensions.bzl", _elixir = "elixir")

elixir = _elixir
hex = _hex
hex_package = _hex_package
