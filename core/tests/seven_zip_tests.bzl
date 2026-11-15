"""Shared 7-Zip extractor pin tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:host.bzl", "host_cell_of")
load("//qt/private/toolchain:seven_zip.bzl", "seven_zip_extractors")

_SHA256 = "0" * 64

_WINDOWS = {"archive": None, "binary": "7zr.exe", "sha256": _SHA256, "urls": ["https://example.com/7zr.exe", "https://mirror.example.com/7zr.exe"]}

_PINS = {
    "seven_zip": {
        "linux_x86_64": {"archive": "7z-linux-x64.tar.xz", "binary": "7zzs", "sha256": _SHA256, "urls": ["https://example.com/7z-linux-x64.tar.xz"]},
        "windows_arm64": _WINDOWS,
        "windows_x86_64": _WINDOWS,
    },
}

def _checks():
    extractors = seven_zip_extractors(_PINS)
    return [
        expect_equal("pinned hosts", sorted(extractors.keys()), ["linux_x86_64", "windows_arm64", "windows_x86_64"]),
        expect_equal("archive extractor", extractors["linux_x86_64"], struct(archive = "7z-linux-x64.tar.xz", binary = "7zzs", sha256 = _SHA256, url = "https://example.com/7z-linux-x64.tar.xz")),
        expect_equal("single file extractor takes the first url", extractors["windows_x86_64"], struct(archive = None, binary = "7zr.exe", sha256 = _SHA256, url = "https://example.com/7zr.exe")),
        expect_equal("windows arm64 runs the x86 binary", extractors["windows_arm64"], extractors["windows_x86_64"]),
        expect_equal("hosts", [
            host_cell_of("Windows 11", "amd64"),
            host_cell_of("windows", "aarch64"),
            host_cell_of("mac os x", "aarch64"),
            host_cell_of("linux", "amd64"),
        ], ["windows_x86_64", "windows_arm64", "macos_arm64", "linux_x86_64"]),
    ]

def seven_zip_tests(name):
    """Declares the shared 7-Zip pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
