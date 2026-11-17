"""Public Elixir module extensions."""

load("//elixir/private/lock:hub.bzl", _hex = "hex")
load("//elixir/private/toolchain:extensions.bzl", _elixir = "elixir")

elixir = _elixir
hex = _hex
