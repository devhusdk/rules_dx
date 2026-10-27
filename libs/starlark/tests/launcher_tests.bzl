"""Unit tests for the Windows launcher text."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:launcher.bzl", "windows_launcher_body", "windows_sh_path_error")

def _launcher_checks():
    git = windows_launcher_body("_main/pkg/t.sh", "D:/tools/git/usr/bin/bash.exe")
    spaced = windows_launcher_body("_main/pkg/t.sh", "C:/Program Files/Git/bin/bash.exe")
    empty = windows_launcher_body("_main/pkg/t.sh", "")
    return [
        expect_equal("toolchain path is written with backslashes", 'set "DX_SH=D:\\tools\\git\\usr\\bin\\bash.exe"' in git, True),
        expect_equal("spaced path stays inside quotes", 'set "DX_SH=C:\\Program Files\\Git\\bin\\bash.exe"' in spaced, True),
        expect_equal("no hardcoded Program Files", "Program Files" in git, False),
        expect_equal("empty path has no assignment", "if not exist \"!DX_SH!\"" in empty, False),
        expect_equal("empty path keeps the BAZEL_SH check", "if defined BAZEL_SH" in empty, True),
        expect_equal("empty path keeps the sh fallback", "\r\nsh \"%SCRIPT:\\=/%\" %*" in empty, True),
        expect_equal("only CRLF line breaks", git.replace("\r\n", "").find("\n"), -1),
        expect_equal("manifest lookup is present", "findstr.exe /b /l" in git, True),
        expect_equal("runner key is present", 'set "RUNNER=_main/pkg/t.sh"' in git, True),
        expect_equal("usr bin is derived from the shell dir", 'set "PATH=!DX_DIR!;!DX_DIR!..\\usr\\bin;%PATH%"' in git, True),
    ]

def _error_checks():
    return [
        expect_equal("normal path is fine", windows_sh_path_error("C:/Program Files/Git/bin/bash.exe"), ""),
        expect_equal("empty path is fine", windows_sh_path_error(""), ""),
        expect_equal("quote is rejected", windows_sh_path_error('C:/a"b/sh') != "", True),
        expect_equal("percent is rejected", windows_sh_path_error("C:/a%b/sh") != "", True),
        expect_equal("bang is rejected", windows_sh_path_error("C:/a!b/sh") != "", True),
    ]

def launcher_unit_tests(name):
    """Declares unit tests for the Windows launcher text."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = _launcher_checks() + _error_checks(),
    )
