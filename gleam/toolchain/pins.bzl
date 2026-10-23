"""Pinned Gleam compiler, Hex packages and rules_gleam identities."""

GLEAM_VERSION = "1.19.2"

GLEAM_RELEASES = "https://github.com/gleam-lang/gleam/releases/download/v" + GLEAM_VERSION + "/"

GLEAM_CELLS = {
    "linux_amd64": struct(
        archive = "gleam-v" + GLEAM_VERSION + "-x86_64-unknown-linux-musl.tar.gz",
        sha256 = "2ba2afb66ca9f6dcc01a4a6e9169b6942963bad6483bec9571ddc078cfe6f41f",
    ),
}

GLEAM_STDLIB_VERSION = "1.0.5"

GLEAM_STDLIB_SHA256 = "cee5b6c076a85b45f60c585f4316c63ec8b7127c119d5738c3958a9c4d50404e"

GLEEUNIT_VERSION = "1.11.0"

GLEEUNIT_SHA256 = "ec31aba74256aea531edf8169931d775bbb384fed0a8a1bdc4dd9354e3e21826"

RULES_GLEAM_COMMIT = "836ee6664660aef8bacb9b1be3cdc8d26a37ae35"

RULES_GLEAM_SHA256 = "2e0ba56610826dc81511f4af73e0d03d64e8e3bf501801e494079ed57c88e78d"

RULES_GLEAM_URL = "https://github.com/iocat/rules_gleam/archive/" + RULES_GLEAM_COMMIT + ".tar.gz"
