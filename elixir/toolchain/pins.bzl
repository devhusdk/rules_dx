"""Pinned Elixir distribution and rules_elixir identities."""

ELIXIR_VERSION = "1.20.4"

ELIXIR_OTP_MAJOR = "29"

ELIXIR_SHA256 = "7863c546cda13fecc949e562e326042451dacf8fd8698a36783cb71eeb223b46"

ELIXIR_URL = "https://builds.hex.pm/builds/elixir/v" + ELIXIR_VERSION + "-otp-" + ELIXIR_OTP_MAJOR + ".zip"

ELIXIR_CELLS = ["linux_amd64"]

ELIXIR_APPLICATIONS = ["eex", "elixir", "ex_unit", "iex", "logger", "mix"]

RULES_ELIXIR_COMMIT = "8e13ada66e2abcea001256346acaeb06f97f1eb5"

RULES_ELIXIR_SHA256 = "eae82d40b892603a5d6243b83a6f6096d8da023ed2b7f450e2648b36d9105b4d"

RULES_ELIXIR_URL = "https://github.com/bazelverse/rules_elixir/archive/" + RULES_ELIXIR_COMMIT + ".tar.gz"
