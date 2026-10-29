"""Sorbet pin tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "SORBET_CELLS", "SORBET_VERSION", "sorbet_url")
load(":repos.bzl", "host_cell", "sorbet_build")

def _checks():
    return [
        expect_equal("sorbet version", SORBET_VERSION, "0.6.13599"),
        expect_equal("pinned cells", sorted(SORBET_CELLS.keys()), ["darwin_aarch64", "darwin_x86_64", "linux_aarch64", "linux_x86_64"]),
        expect_true("digests are sha256", all([len(c.sha256) == 64 for c in SORBET_CELLS.values()])),
        expect_true("gems name the version", all([SORBET_VERSION in c.gem and c.gem.endswith(".gem") for c in SORBET_CELLS.values()])),
        expect_equal("gem url", sorbet_url("sorbet-static-1-x86_64-linux.gem"), "https://rubygems.org/downloads/sorbet-static-1-x86_64-linux.gem"),
        expect_equal("linux host", host_cell("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell("mac os x", "aarch64"), "darwin_aarch64"),
        expect_equal("windows host", host_cell("windows server 2022", "amd64"), "windows_x86_64"),
        expect_contains("the repo exports the binary", sorbet_build("libexec/sorbet"), 'exports_files(["libexec/sorbet"])'),
    ]

def ruby_metadata_tests(name):
    """Declares the Sorbet pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
