"""Pinned Erlang/OTP runtime, native closure and rules_erlang identities."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME", "JAMMY_RUNTIME_MIRROR")

_PINS = DATA["dx_erlang"]

_ARCHIVES = _PINS["archives"]

OTP_VERSION = _PINS["version"]

# Moves with OTP_VERSION: the erts-* directory of the pinned OTP archive.
ERTS_VERSION = _PINS["erts_version"]

OTP_RELEASE = _PINS["otp_release"]

RULES_ERLANG_COMMIT = _ARCHIVES["rules_erlang"]["version"]

RULES_ERLANG_SHA256 = _ARCHIVES["rules_erlang"]["sha256"]

RULES_ERLANG_URLS = _ARCHIVES["rules_erlang"]["urls"]

ERLFMT_VERSION = _ARCHIVES["dx_erlfmt"]["version"]

ELVIS_VERSION = _ARCHIVES["dx_elvis"]["version"]

ELVIS_CORE_VERSION = _ARCHIVES["dx_elvis_core"]["version"]

KATANA_CODE_VERSION = _ARCHIVES["dx_katana_code"]["version"]

GETOPT_VERSION = _ARCHIVES["dx_erlang_getopt"]["version"]

TOOL_SOURCES = {
    "dx_elvis": struct(
        dropped = ["src/elvis.app.src"],
        patches = [Label("//patches/elvis:lazy_format.patch")],
        sha256 = _ARCHIVES["dx_elvis"]["sha256"],
        strip_prefix = _ARCHIVES["dx_elvis"]["strip_prefix"],
        urls = _ARCHIVES["dx_elvis"]["urls"],
    ),
    "dx_elvis_core": struct(
        dropped = ["src/elvis_core.app.src"],
        patches = [],
        sha256 = _ARCHIVES["dx_elvis_core"]["sha256"],
        strip_prefix = _ARCHIVES["dx_elvis_core"]["strip_prefix"],
        urls = _ARCHIVES["dx_elvis_core"]["urls"],
    ),
    "dx_erlfmt": struct(
        dropped = ["src/rebar3_fmt_prv.erl"],
        patches = [],
        sha256 = _ARCHIVES["dx_erlfmt"]["sha256"],
        strip_prefix = _ARCHIVES["dx_erlfmt"]["strip_prefix"],
        urls = _ARCHIVES["dx_erlfmt"]["urls"],
    ),
    "dx_erlang_getopt": struct(
        dropped = ["ebin"],
        patches = [],
        sha256 = _ARCHIVES["dx_erlang_getopt"]["sha256"],
        strip_prefix = _ARCHIVES["dx_erlang_getopt"]["strip_prefix"],
        urls = _ARCHIVES["dx_erlang_getopt"]["urls"],
    ),
    "dx_katana_code": struct(
        dropped = ["src/katana_code.app.src"],
        patches = [],
        sha256 = _ARCHIVES["dx_katana_code"]["sha256"],
        strip_prefix = _ARCHIVES["dx_katana_code"]["strip_prefix"],
        urls = _ARCHIVES["dx_katana_code"]["urls"],
    ),
}

# Hex releases fetched for erlang_bench: erlperf and its one dependency.
HEX_SOURCES = {
    name: struct(package = pin["package"], sha256 = pin["sha256"], version = pin["version"])
    for name, pin in _PINS["hex"].items()
}

GLIBC_FLOOR = "2.35"

RUNTIME_MIRROR = JAMMY_RUNTIME_MIRROR

def _cell(cell):
    return _PINS["cells"][cell]

CELLS = {
    "linux_x86_64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        otp_sha256 = _cell("linux_x86_64")["sha256"],
        otp_urls = _cell("linux_x86_64")["urls"],
        strip_prefix = _cell("linux_x86_64")["strip_prefix"],
        runtime = JAMMY_RUNTIME["linux_x86_64"],
    ),
    "linux_arm64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        otp_sha256 = _cell("linux_arm64")["sha256"],
        otp_urls = _cell("linux_arm64")["urls"],
        strip_prefix = _cell("linux_arm64")["strip_prefix"],
        runtime = JAMMY_RUNTIME["linux_arm64"],
    ),
    "macos_arm64": struct(
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        otp_sha256 = _cell("macos_arm64")["sha256"],
        otp_urls = _cell("macos_arm64")["urls"],
        strip_prefix = _cell("macos_arm64")["strip_prefix"],
        runtime = [],
    ),
    "windows_x86_64": struct(
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        otp_sha256 = _cell("windows_x86_64")["sha256"],
        otp_urls = _cell("windows_x86_64")["urls"],
        strip_prefix = _cell("windows_x86_64")["strip_prefix"],
        runtime = [],
    ),
    # No arm64 OTP is published, so this is the x64 build running under Windows emulation.
    "windows_arm64": struct(
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        otp_sha256 = _cell("windows_arm64")["sha256"],
        otp_urls = _cell("windows_arm64")["urls"],
        strip_prefix = _cell("windows_arm64")["strip_prefix"],
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
