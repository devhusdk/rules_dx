"""Sorbet pin tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//libs/starlark:host.bzl", "host_cell_of")
load(":pins.bzl", "SORBET_CELLS", "SORBET_VERSION", "sorbet_url")
load(":repos.bzl", "sorbet_build")

def _checks():
    return [
        expect_equal("sorbet version", SORBET_VERSION, "0.6.13599"),
        expect_equal("pinned cells", sorted(SORBET_CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64"]),
        expect_true("digests are sha256", all([len(c.sha256) == 64 for c in SORBET_CELLS.values()])),
        expect_true("gems name the version", all([SORBET_VERSION in c.gem and c.gem.endswith(".gem") for c in SORBET_CELLS.values()])),
        expect_equal("gem url", sorbet_url("sorbet-static-1-x86_64-linux.gem"), "https://rubygems.org/downloads/sorbet-static-1-x86_64-linux.gem"),
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
        expect_contains("the repo exports the binary", sorbet_build("libexec/sorbet"), 'exports_files(["libexec/sorbet"])'),
    ]

def ruby_metadata_tests(name):
    """Declares the Sorbet pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )

def ruby_crlf_tests(name):
    """Declares the test that the patched rules_ruby batch template keeps CRLF endings."""
    starlark_test(
        name = name,
        mode = "execution",
        file_line_endings = {"@rules_ruby//ruby/private/binary:binary.cmd.tpl": "crlf"},
        file_blocks = {
            "@rules_ruby//ruby/private/binary:binary.cmd.tpl": [
                'if "%COVERAGE%" == "1" (\r\n  call :rlocation {coverage_helper} _coverage_helper_path\r\n',
                "\r\n{bundler_command} {ruby_binary_name} {binary} {args} %*\r\n",
            ],
        },
    )
