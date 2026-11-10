"""Pinned Flang compiler, flang-rt sources and rules_fortran identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_fortran"]

FLANG_VERSION = _PINS["version"]

RULES_FORTRAN_COMMIT = _PINS["archives"]["rules_fortran"]["version"]

RULES_FORTRAN_SHA256 = _PINS["archives"]["rules_fortran"]["sha256"]

RULES_FORTRAN_URLS = _PINS["archives"]["rules_fortran"]["urls"]

FLANG_RT_URLS = _PINS["archives"]["flang_rt"]["urls"]

FLANG_RT_SHA256 = _PINS["archives"]["flang_rt"]["sha256"]

FLANG_RT_PREFIX = _PINS["archives"]["flang_rt"]["strip_prefix"]

_CONSTRAINTS = {
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

FORTRAN_CELLS = {
    cell: struct(
        constraints = _CONSTRAINTS[cell],
        sha256 = pin["sha256"],
        strip_prefix = pin["strip_prefix"],
        urls = pin["urls"],
        **({"exe": ".exe"} if cell.startswith("windows") else {})
    )
    for cell, pin in _PINS["cells"].items()
}
