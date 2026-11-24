"""Pinned Elixir distribution identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_elixir"]

ELIXIR_VERSION = _PINS["version"]

ELIXIR_OTP_MAJOR = _PINS["otp_major"]

ELIXIR_SHA256 = _PINS["archives"]["elixir"]["sha256"]

ELIXIR_URLS = _PINS["archives"]["elixir"]["urls"]

ELIXIR_CELLS = ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]

ELIXIR_APPLICATIONS = ["eex", "elixir", "ex_unit", "iex", "logger", "mix"]

DIALYZER_PLT_ELIXIR = ["elixir", "logger"]

CREDO_PACKAGES = {
    name: struct(sha256 = pin["sha256"], version = pin["version"])
    for name, pin in _PINS["hex"].items()
}
