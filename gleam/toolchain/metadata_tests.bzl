"""Gleam toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":gleam.bzl", "otp_tool_paths")
load(":pins.bzl", "GLEAM_CELLS", "GLEAM_RELEASES", "GLEAM_STDLIB_SHA256", "GLEAM_STDLIB_VERSION", "GLEAM_VERSION", "GLEEUNIT_SHA256", "GLEEUNIT_VERSION", "RULES_GLEAM_COMMIT", "RULES_GLEAM_SHA256", "RULES_GLEAM_URL")
load(":repos.bzl", "DROPPED_PATHS")

def _checks():
    linux = GLEAM_CELLS["linux_amd64"]
    return [
        expect_equal("gleam version", GLEAM_VERSION, "1.19.2"),
        expect_equal("release url", GLEAM_RELEASES, "https://github.com/gleam-lang/gleam/releases/download/v1.19.2/"),
        expect_equal("pinned cells", sorted(GLEAM_CELLS.keys()), ["linux_amd64"]),
        expect_equal("linux archive", linux.archive, "gleam-v1.19.2-x86_64-unknown-linux-musl.tar.gz"),
        expect_equal("linux digest", len(linux.sha256), 64),
        expect_equal("gleam_stdlib version", GLEAM_STDLIB_VERSION, "1.0.5"),
        expect_equal("gleam_stdlib digest", len(GLEAM_STDLIB_SHA256), 64),
        expect_equal("gleeunit version", GLEEUNIT_VERSION, "1.11.0"),
        expect_equal("gleeunit digest", len(GLEEUNIT_SHA256), 64),
        expect_equal("rules_gleam commit", len(RULES_GLEAM_COMMIT), 40),
        expect_equal("rules_gleam digest", len(RULES_GLEAM_SHA256), 64),
        expect_contains("rules_gleam url names the commit", RULES_GLEAM_URL, RULES_GLEAM_COMMIT),
        expect_true("the unpatched launcher template is dropped", "gleam/templates/[[main_module]].sh" in DROPPED_PATHS),
        expect_true("the upstream compiler download is dropped", "gleam_tools/repositories.bzl" in DROPPED_PATHS),
        expect_true("the upstream Hex fetcher is dropped", "gleam_hex" in DROPPED_PATHS),
        expect_equal("otp tools come from the OTP tree", otp_tool_paths("/otp"), {"erl": "/otp/bin/erl", "erlc": "/otp/bin/erlc", "escript": "/otp/bin/escript"}),
    ]

def gleam_metadata_tests(name):
    """Declares the Gleam toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
