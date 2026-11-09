"""Sorbet pin tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//libs/starlark:host.bzl", "host_cell_of")

def _checks():
    return [
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
        expect_equal("sorbet pinned cells", sorted(PINS["dx_sorbet"]["cells"].keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64", "windows_x86_64"]),
        expect_true("sorbet gems", all([c["type"] == "gem" for c in PINS["dx_sorbet"]["cells"].values() if "urls" in c])),
        expect_true("sorbet windows stub", "stub" in PINS["dx_sorbet"]["cells"]["windows_x86_64"]),
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
