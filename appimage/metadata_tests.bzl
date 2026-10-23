"""AppImage pin, host and name-rule coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "name_errors")
load(
    ":pins.bzl",
    "MKSQUASHFS_VERSION",
    "RUNTIME_SHA256",
    "RUNTIME_URL",
    "RUNTIME_VERSION",
    "SUPPORTED_CELLS",
    "TOOL_BINARIES",
    "TOOL_SHA256",
    "TOOL_URL",
    "TOOL_VERSION",
    "UNSUPPORTED_CELLS",
)
load(":repos.bzl", "host_error", "hub_build")

def _checks():
    return [
        expect_equal("tool version", TOOL_VERSION, "1.9.1"),
        expect_equal("runtime version", RUNTIME_VERSION, "20251108"),
        expect_equal("mksquashfs version", MKSQUASHFS_VERSION, "4.6.1"),
        expect_contains("tool url carries the version", TOOL_URL, "/" + TOOL_VERSION + "/"),
        expect_contains("runtime url carries the version", RUNTIME_URL, "/" + RUNTIME_VERSION + "/"),
        expect_equal("tool digest length", len(TOOL_SHA256), 64),
        expect_equal("runtime digest length", len(RUNTIME_SHA256), 64),
        expect_equal("tool binaries", TOOL_BINARIES, ["appimagetool", "desktop-file-validate", "mksquashfs"]),
        expect_equal("supported cells", SUPPORTED_CELLS, ["linux_x86_64"]),
        expect_equal("unsupported cells", sorted(UNSUPPORTED_CELLS.keys()), ["linux_arm64"]),
        expect_equal("linux x86_64 host", host_error("linux", "amd64"), ""),
        expect_contains("arm64 host", host_error("linux", "aarch64"), "only pinned for linux_x86_64"),
        expect_contains("windows host", host_error("windows 11", "amd64"), "only pinned for linux_x86_64"),
        expect_contains("hub exposes the tools", hub_build(), '"bin/appimagetool"'),
        expect_contains("hub exposes the runtime", hub_build(), 'name = "runtime_file"'),
        expect_equal("matching names", name_errors("app", "app.desktop", "app.png"), []),
        expect_equal("svg icon", name_errors("app", "app.desktop", "app.svg"), []),
        expect_true("wrong desktop name", len(name_errors("app", "other.desktop", "app.png")) == 1),
        expect_true("wrong icon name", len(name_errors("app", "app.desktop", "other.png")) == 1),
    ]

def appimage_metadata_tests(name):
    """Declares the AppImage metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
