"""Pinned Elixir distribution and rules_elixir identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_elixir"]

ELIXIR_VERSION = _PINS["version"]

ELIXIR_OTP_MAJOR = _PINS["otp_major"]

ELIXIR_SHA256 = _PINS["archives"]["elixir"]["sha256"]

ELIXIR_URLS = _PINS["archives"]["elixir"]["urls"]

ELIXIR_CELLS = ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]

ELIXIR_APPLICATIONS = ["eex", "elixir", "ex_unit", "iex", "logger", "mix"]

# The Elixir applications the dialyzer PLT covers, on top of the OTP core PLT. A call into an
# application missing here is an "Unknown function" warning. `elixir` also holds the compiler
# (elixir_erl) dialyzer loads to read the debug info of every Elixir beam.
DIALYZER_PLT_ELIXIR = ["elixir", "logger"]

RULES_ELIXIR_COMMIT = _PINS["archives"]["rules_elixir"]["version"]

RULES_ELIXIR_SHA256 = _PINS["archives"]["rules_elixir"]["sha256"]

RULES_ELIXIR_URLS = _PINS["archives"]["rules_elixir"]["urls"]

RULES_ELIXIR_DROPPED = [".bazelignore", ".bazelrc", ".bazelversion", "MODULE.bazel", "examples", "test"]

# Credo and its dependencies, built from the Hex tarballs (#2349).
CREDO_PACKAGES = {
    name: struct(sha256 = pin["sha256"], version = pin["version"])
    for name, pin in _PINS["hex"].items()
}
