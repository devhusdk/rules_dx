"""Erlang toolchain pin and launcher tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":pins.bzl", "CELLS", "DIALYZER_PLT_APPS", "DIALYZER_PLT_CORE", "ELVIS_CORE_VERSION", "ELVIS_VERSION", "ERTS_VERSION", "GETOPT_VERSION", "GLIBC_FLOOR", "OTP_VERSION", "TOOL_SOURCES")
load(":repos.bzl", "CLOSURE_DIR", "RESTORE_VARIABLE", "TOOL_BUILDS", "dropped_application", "erl_script", "nif_compatible_cases", "nif_header_cases", "nif_include_dir", "replace_once")

_SOURCE = """#!/bin/sh
ROOTDIR=%FINAL_ROOTDIR%
export PROGNAME
exec "$BINDIR/erlexec" ${1+"$@"}
"""

def _checks():
    script = erl_script(_SOURCE, CLOSURE_DIR)
    plain = erl_script(_SOURCE, None)
    cell = CELLS["linux_amd64"]
    arm = CELLS["linux_arm64"]
    packages = [package.package for package in cell.runtime]
    debs = [package for name in CELLS for package in CELLS[name].runtime]
    arm_paths = [path for package in arm.runtime for path in package.libraries.values()]
    return [
        expect_equal("otp version", OTP_VERSION, "29.1.2"),
        expect_equal("glibc floor", GLIBC_FLOOR, "2.35"),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_amd64", "linux_arm64", "macos_arm64"]),
        expect_equal("runtime packages", packages, ["libtinfo6", "zlib1g", "gcc-12-base", "libgcc-s1", "libstdc++6", "libssl3"]),
        expect_equal("arm64 ships the same packages", [package.package for package in arm.runtime], packages),
        expect_equal("arm64 libraries are aarch64", [path for path in arm_paths if "/aarch64-linux-gnu/" not in path], []),
        expect_equal("arm64 debs are arm64", [package.deb for package in arm.runtime if not package.deb.endswith("_arm64.deb")], []),
        expect_equal("macOS needs no closure", CELLS["macos_arm64"].runtime, []),
        expect_equal("every archive pins a digest", [name for name in CELLS if len(CELLS[name].otp_sha256) != 64], []),
        expect_equal("every archive is this release", [name for name in CELLS if OTP_VERSION not in CELLS[name].otp_url], []),
        expect_equal("every package pins a digest", [package.deb for package in debs if len(package.sha256) != 64], []),
        expect_equal("every library ships a license", [package.deb for package in debs if package.libraries and package.license == ""], []),
        expect_true("rootdir is relocatable", "/nonexistent" in script and "%FINAL_ROOTDIR%" not in script),
        expect_contains("closure joins the loader path", script, 'LD_LIBRARY_PATH="$ROOTDIR/dx_closure/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"'),
        expect_contains("caller path is saved", script, RESTORE_VARIABLE + '="${LD_LIBRARY_PATH-}"'),
        expect_contains("emulator restores the caller path", script, "exec \"$BINDIR/erlexec\" -eval 'case os:getenv(\"" + RESTORE_VARIABLE + "\")"),
        expect_contains("restore clears an empty path", script, 'os:unsetenv("LD_LIBRARY_PATH")'),
        expect_false("launcher keeps no plain exec", 'exec "$BINDIR/erlexec" ${1' in script),
        expect_true("no closure keeps the plain exec", 'exec "$BINDIR/erlexec" ${1+"$@"}' in plain),
        expect_false("no closure leaves the loader path", "LD_LIBRARY_PATH" in plain),
        expect_true("no closure is still relocatable", "/nonexistent" in plain and "%FINAL_ROOTDIR%" not in plain),
        expect_equal("every tool source pins a digest", [name for name in TOOL_SOURCES if len(TOOL_SOURCES[name].sha256) != 64 or TOOL_SOURCES[name].sha256.strip("0123456789abcdef") != ""], []),
        expect_equal("every tool source has a build", sorted(TOOL_BUILDS.keys()), sorted(TOOL_SOURCES.keys())),
        expect_equal("elvis and elvis_core move together", ELVIS_VERSION, ELVIS_CORE_VERSION),
        expect_equal("elvis shares the getopt pin", GETOPT_VERSION, "1.0.3"),
        expect_contains("elvis links the shared getopt", TOOL_BUILDS["dx_elvis"], "@dx_erlang_getopt//:erlang_app"),
        expect_contains("erlfmt links the shared getopt", TOOL_BUILDS["dx_erlfmt"], "@dx_erlang_getopt//:erlang_app"),
        expect_equal("dialyzer PLT apps are shipped", [app for app in DIALYZER_PLT_CORE + DIALYZER_PLT_APPS if dropped_application(app + "-1.0")], []),
        expect_equal("dialyzer PLT apps are listed once", len(DIALYZER_PLT_CORE + DIALYZER_PLT_APPS), len(depset(DIALYZER_PLT_CORE + DIALYZER_PLT_APPS).to_list())),
        expect_equal("nif headers cover every cell", sorted([key.rpartition("cell_")[2] for key in nif_header_cases(CELLS.keys()) if "cell_" in key]), sorted(CELLS.keys())),
        expect_equal("nif headers resolve to the cell repo", nif_header_cases(["linux_amd64"])["@rules_dx//erlang/toolchain:cell_linux_amd64"], ["@dx_otp_linux_amd64//:erl_nif"]),
        expect_equal("nif headers default to nothing", nif_header_cases(CELLS.keys())["//conditions:default"], []),
        expect_equal("nif default is incompatible", nif_compatible_cases(CELLS.keys())["//conditions:default"], ["@platforms//:incompatible"]),
        expect_equal("nif include follows erts", nif_include_dir(), "erts-" + ERTS_VERSION + "/include"),
        expect_equal("replace_once edits", replace_once("a(F) b", [("a(F)", "c(F)")], "x"), "c(F) b"),
        expect_true("wx is dropped", dropped_application("wx-2.5")),
        expect_false("crypto is kept", dropped_application("crypto-5.6")),
        expect_false("prefix is not enough", dropped_application("wxwidgets-1.0")),
    ]

def erlang_metadata_tests(name):
    """Declares the Erlang toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
