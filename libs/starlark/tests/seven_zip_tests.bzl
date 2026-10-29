"""Shared 7-Zip extractor pin tests."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//libs/starlark:seven_zip.bzl", "SEVEN_ZIP_EXTRACTORS", "seven_zip_host")

_HEX = "0123456789abcdef"

def _is_sha256(value):
    return len(value) == 64 and all([c in _HEX for c in value.elems()])

def _checks():
    return [
        expect_equal("pinned hosts", sorted(SEVEN_ZIP_EXTRACTORS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]),
        expect_true("https urls", all([e.url.startswith("https://") for e in SEVEN_ZIP_EXTRACTORS.values()])),
        expect_true("sha256 digests", all([_is_sha256(e.sha256) for e in SEVEN_ZIP_EXTRACTORS.values()])),
        expect_equal("windows hosts are single files", [SEVEN_ZIP_EXTRACTORS[h].archive for h in ["windows_arm64", "windows_x86_64"]], [None, None]),
        expect_equal("windows binary", SEVEN_ZIP_EXTRACTORS["windows_x86_64"].binary, "7zr.exe"),
        expect_equal("windows arm64 runs the x86 binary", SEVEN_ZIP_EXTRACTORS["windows_arm64"], SEVEN_ZIP_EXTRACTORS["windows_x86_64"]),
        expect_equal("hosts", [
            seven_zip_host("Windows 11", "amd64"),
            seven_zip_host("windows", "aarch64"),
            seven_zip_host("mac os x", "aarch64"),
            seven_zip_host("linux", "amd64"),
        ], ["windows_x86_64", "windows_arm64", "macos_arm64", "linux_x86_64"]),
    ]

def seven_zip_tests(name):
    """Declares the shared 7-Zip pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
