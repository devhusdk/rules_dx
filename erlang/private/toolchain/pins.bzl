"""Pinned Erlang/OTP runtime, native closure and rules_erlang identities."""

load("//libs/starlark:jammy_runtime.bzl", "JAMMY_RUNTIME", "JAMMY_RUNTIME_MIRROR")

OTP_VERSION = "29.1.2"

# Moves with OTP_VERSION: the erts-* directory of the pinned OTP archive.
ERTS_VERSION = "17.1.1"

OTP_RELEASE = "29"

RULES_ERLANG_COMMIT = "13977b3309fd0b0d6795f322cffd86a8c161be64"

RULES_ERLANG_SHA256 = "830823cc562431cb61d7787bc716f3fa8101ef3afe57350c40f92379f262fde9"

RULES_ERLANG_URL = "https://github.com/bazelverse/rules_erlang/archive/" + RULES_ERLANG_COMMIT + ".tar.gz"

ERLFMT_VERSION = "1.8.0"

ELVIS_VERSION = "6.0.0"

ELVIS_CORE_VERSION = "6.0.0"

KATANA_CODE_VERSION = "2.5.3"

GETOPT_VERSION = "1.0.3"

TOOL_SOURCES = {
    "dx_elvis": struct(
        dropped = ["src/elvis.app.src"],
        patches = [Label("//patches/elvis:lazy_format.patch")],
        sha256 = "ea0d3438062d94b686b375e98995584ed7b6b8863582f77b1966971299e400f4",
        strip_prefix = "elvis-" + ELVIS_VERSION,
        url = "https://github.com/inaka/elvis/archive/refs/tags/" + ELVIS_VERSION + ".tar.gz",
    ),
    "dx_elvis_core": struct(
        dropped = ["src/elvis_core.app.src"],
        patches = [],
        sha256 = "b93a464eb500e9e3f199d0dec280af6f2000335c0e0c8bb21af3782119fc5b73",
        strip_prefix = "elvis_core-" + ELVIS_CORE_VERSION,
        url = "https://github.com/inaka/elvis_core/archive/refs/tags/" + ELVIS_CORE_VERSION + ".tar.gz",
    ),
    "dx_erlfmt": struct(
        dropped = ["src/rebar3_fmt_prv.erl"],
        patches = [],
        sha256 = "c7205f3a28dac589c623f21546d63d5b04cc669545d03485cb88552de0117f43",
        strip_prefix = "erlfmt-" + ERLFMT_VERSION,
        url = "https://github.com/WhatsApp/erlfmt/archive/refs/tags/v" + ERLFMT_VERSION + ".tar.gz",
    ),
    "dx_erlang_getopt": struct(
        dropped = ["ebin"],
        patches = [],
        sha256 = "e5a4ec9c6312ec5a850dea13ffa2cd50850e694fd79597cf76acb548287134d9",
        strip_prefix = "getopt-" + GETOPT_VERSION,
        url = "https://github.com/jcomellas/getopt/archive/refs/tags/v" + GETOPT_VERSION + ".tar.gz",
    ),
    "dx_katana_code": struct(
        dropped = ["src/katana_code.app.src"],
        patches = [],
        sha256 = "da09a4b30d191bc000d3fbe8fd08e4161acc43a5cfc67c123c07b78b75562133",
        strip_prefix = "katana-code-" + KATANA_CODE_VERSION,
        url = "https://github.com/inaka/katana-code/archive/refs/tags/" + KATANA_CODE_VERSION + ".tar.gz",
    ),
}

# Hex releases fetched for erlang_bench: erlperf and its one dependency.
HEX_SOURCES = {
    "dx_erlang_argparse": struct(
        package = "argparse",
        sha256 = "525979122bea3641a1dd3abc53f2add19f7f427d507018f8c7caf0693a6e78c8",
        version = "2.0.0",
    ),
    "dx_erlperf": struct(
        package = "erlperf",
        sha256 = "c6fc9b9c784facaca01b826b7721efb3bcd7675b228902a182d9e0816f880029",
        version = "2.3.0",
    ),
}

GLIBC_FLOOR = "2.35"

RUNTIME_MIRROR = JAMMY_RUNTIME_MIRROR

_WINDOWS_OTP_URL = "https://github.com/erlang/otp/releases/download/OTP-" + OTP_VERSION + "/otp_win64_" + OTP_VERSION + ".zip"

_WINDOWS_OTP_SHA256 = "4efe9772a84c6c1ef75377e9326815584958776c905f2bf687b62509e2d809c0"

CELLS = {
    "linux_x86_64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        otp_sha256 = "1df9e6285b4d14319a789176db60c273131abebdfa6b5474a084dba49b0e54a2",
        otp_url = "https://builds.hex.pm/builds/otp/amd64/ubuntu-22.04/OTP-" + OTP_VERSION + ".tar.gz",
        strip_prefix = "OTP-" + OTP_VERSION,
        runtime = JAMMY_RUNTIME["linux_x86_64"],
    ),
    "linux_arm64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        otp_sha256 = "8e16954a07278e83de260489ed4af9bcca0e22ab7e99eb11de781ec6f01e95d9",
        otp_url = "https://builds.hex.pm/builds/otp/arm64/ubuntu-22.04/OTP-" + OTP_VERSION + ".tar.gz",
        strip_prefix = "OTP-" + OTP_VERSION,
        runtime = JAMMY_RUNTIME["linux_arm64"],
    ),
    "macos_arm64": struct(
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        otp_sha256 = "579af3bb043e0692503f7f24c6e65b620bdf1802a13567a7247d1d18f1ec5470",
        otp_url = "https://github.com/erlef/otp_builds/releases/download/OTP-" + OTP_VERSION + "/OTP-" + OTP_VERSION + "-macos-arm64.tar.gz",
        strip_prefix = "",
        runtime = [],
    ),
    "windows_x86_64": struct(
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        otp_sha256 = _WINDOWS_OTP_SHA256,
        otp_url = _WINDOWS_OTP_URL,
        strip_prefix = "",
        runtime = [],
    ),
    # No arm64 OTP is published, so this is the x64 build running under Windows emulation.
    "windows_arm64": struct(
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        otp_sha256 = _WINDOWS_OTP_SHA256,
        otp_url = _WINDOWS_OTP_URL,
        strip_prefix = "",
        runtime = [],
    ),
}

# Cells whose OTP is pinned but cannot run Erlang yet: the launchers are POSIX sh (#5962).
# Apps do not build for them until those land.
OTP_UNPORTED_CELLS = []

OTP_NIF_UNPORTED_CELLS = []

OTP_DIALYZER_UNPORTED_CELLS = []

# The OTP applications the dialyzer base PLT covers. A call into an application missing here
# is an "Unknown function" warning, so the list holds what Erlang apps commonly use.
DIALYZER_PLT_CORE = ["erts", "kernel", "stdlib"]

DIALYZER_PLT_APPS = ["compiler", "crypto", "syntax_tools", "sasl", "public_key", "ssl", "inets", "tools", "runtime_tools", "eunit"]

DROPPED_PATHS = ["Install", "misc"]

WINDOWS_DROPPED_PATHS = ["doc", "vc_redist.exe"]

DROPPED_APPLICATIONS = ["wx"]

DROPPED_APPLICATION_DIRS = ["doc", "examples", "src"]
