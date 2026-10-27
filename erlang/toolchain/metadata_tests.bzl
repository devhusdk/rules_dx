"""Erlang toolchain pin and launcher tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":pins.bzl", "CELLS", "GLIBC_FLOOR", "OTP_VERSION")
load(":repos.bzl", "CLOSURE_DIR", "RESTORE_VARIABLE", "dropped_application", "erl_script")

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
