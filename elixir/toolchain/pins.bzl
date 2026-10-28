"""Pinned Elixir distribution and rules_elixir identities."""

ELIXIR_VERSION = "1.20.4"

ELIXIR_OTP_MAJOR = "29"

ELIXIR_SHA256 = "7863c546cda13fecc949e562e326042451dacf8fd8698a36783cb71eeb223b46"

ELIXIR_URL = "https://builds.hex.pm/builds/elixir/v" + ELIXIR_VERSION + "-otp-" + ELIXIR_OTP_MAJOR + ".zip"

ELIXIR_CELLS = ["linux_amd64", "linux_arm64", "macos_arm64"]

ELIXIR_APPLICATIONS = ["eex", "elixir", "ex_unit", "iex", "logger", "mix"]

# The Elixir applications the dialyzer PLT covers, on top of the OTP core PLT. A call into an
# application missing here is an "Unknown function" warning. `elixir` also holds the compiler
# (elixir_erl) dialyzer loads to read the debug info of every Elixir beam.
DIALYZER_PLT_ELIXIR = ["elixir", "logger"]

RULES_ELIXIR_COMMIT = "8e13ada66e2abcea001256346acaeb06f97f1eb5"

RULES_ELIXIR_SHA256 = "eae82d40b892603a5d6243b83a6f6096d8da023ed2b7f450e2648b36d9105b4d"

RULES_ELIXIR_URL = "https://github.com/bazelverse/rules_elixir/archive/" + RULES_ELIXIR_COMMIT + ".tar.gz"

# Credo and its dependencies, built from the Hex tarballs (#2349).
CREDO_PACKAGES = {
    "bunt": struct(
        sha256 = "dc5f86aa08a5f6fa6b8096f0735c4e76d54ae5c9fa2c143e5a1fc7c1cd9bb6b5",
        version = "1.0.0",
    ),
    "credo": struct(
        sha256 = "2d8bc95d5a7bb99dd2613621d4f08c6a3575c3fd4b62e6a2b48a100352a557b8",
        version = "1.7.19",
    ),
    "file_system": struct(
        sha256 = "7a15ff97dfe526aeefb090a7a9d3d03aa907e100e262a0f8f7746b78f8f87a5d",
        version = "1.1.1",
    ),
    "jason": struct(
        sha256 = "b0c823996102bcd0239b3c2444eb00409b72f6a140c1950bc8b457d836b30684",
        version = "1.4.5",
    ),
}
