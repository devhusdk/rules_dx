"""Pinned Erlang/OTP runtime and native closure identities."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME", "jammy_deb_urls")

_PINS = DATA["dx_erlang"]

_ARCHIVES = _PINS["archives"]

OTP_VERSION = _PINS["version"]

ERTS_VERSION = _PINS["erts_version"]

OTP_RELEASE = _PINS["otp_release"]

ELVIS_VERSION = _ARCHIVES["dx_elvis"]["version"]

ELVIS_CORE_VERSION = _ARCHIVES["dx_elvis_core"]["version"]

HEX_SOURCES = {
    name: struct(package = pin["package"], sha256 = pin["sha256"], version = pin["version"])
    for name, pin in _PINS["hex"].items()
}

GLIBC_FLOOR = "2.35"

runtime_urls = jammy_deb_urls

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
    "windows_arm64": struct(
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        otp_sha256 = _cell("windows_arm64")["sha256"],
        otp_urls = _cell("windows_arm64")["urls"],
        strip_prefix = _cell("windows_arm64")["strip_prefix"],
        runtime = [],
    ),
}

OTP_UNPORTED_CELLS = []

OTP_NIF_UNPORTED_CELLS = []

OTP_DIALYZER_UNPORTED_CELLS = []

DIALYZER_PLT_CORE = ["erts", "kernel", "stdlib"]

DIALYZER_PLT_APPS = ["compiler", "crypto", "syntax_tools", "sasl", "public_key", "ssl", "inets", "tools", "runtime_tools", "eunit"]

DROPPED_PATHS = ["Install", "misc"]

WINDOWS_DROPPED_PATHS = ["doc", "vc_redist.exe"]

DROPPED_APPLICATIONS = ["wx"]

DROPPED_APPLICATION_DIRS = ["doc", "examples", "src"]
