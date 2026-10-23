"""AppImage pin, host and name-rule coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "name_errors")
load(
    ":pins.bzl",
    "MKSQUASHFS_VERSION",
    "RUNTIMES",
    "RUNTIME_VERSION",
    "SUPPORTED_CELLS",
    "TOOLS",
    "TOOL_BINARIES",
    "TOOL_VERSION",
)
load(":repos.bzl", "host_arch", "host_error", "hub_build")

def _pin_checks():
    checks = []
    for arch in ["aarch64", "x86_64"]:
        checks.extend([
            expect_contains(arch + " tool url carries the version", TOOLS[arch]["url"], "/" + TOOL_VERSION + "/"),
            expect_contains(arch + " tool url names the arch", TOOLS[arch]["url"], arch),
            expect_contains(arch + " runtime url carries the version", RUNTIMES[arch]["url"], "/" + RUNTIME_VERSION + "/"),
            expect_contains(arch + " runtime url names the arch", RUNTIMES[arch]["url"], arch),
            expect_equal(arch + " tool digest length", len(TOOLS[arch]["sha256"]), 64),
            expect_equal(arch + " runtime digest length", len(RUNTIMES[arch]["sha256"]), 64),
        ])
    checks.append(expect_true("arch digests differ", TOOLS["aarch64"]["sha256"] != TOOLS["x86_64"]["sha256"] and RUNTIMES["aarch64"]["sha256"] != RUNTIMES["x86_64"]["sha256"]))
    return checks

def _checks():
    return _pin_checks() + [
        expect_equal("tool version", TOOL_VERSION, "1.9.1"),
        expect_equal("runtime version", RUNTIME_VERSION, "20251108"),
        expect_equal("mksquashfs version", MKSQUASHFS_VERSION, "4.6.1"),
        expect_equal("tool binaries", TOOL_BINARIES, ["appimagetool", "desktop-file-validate", "mksquashfs"]),
        expect_equal("supported cells", SUPPORTED_CELLS, ["linux_arm64", "linux_x86_64"]),
        expect_equal("linux x86_64 host", host_arch("linux", "amd64"), "x86_64"),
        expect_equal("linux arm64 host", host_arch("linux", "aarch64"), "aarch64"),
        expect_equal("linux arm64 host alias", host_arch("linux", "arm64"), "aarch64"),
        expect_equal("linux x86_64 host has no error", host_error("linux", "amd64"), ""),
        expect_equal("linux arm64 host has no error", host_error("linux", "aarch64"), ""),
        expect_contains("riscv host", host_error("linux", "riscv64"), "only pinned for linux_x86_64 and linux_arm64"),
        expect_contains("windows host", host_error("windows 11", "amd64"), "only pinned for linux_x86_64 and linux_arm64"),
        expect_contains("hub exposes the tools", hub_build(), '"bin/appimagetool"'),
        expect_contains("hub exposes the x86_64 runtime", hub_build(), 'name = "runtime_x86_64"'),
        expect_contains("hub exposes the aarch64 runtime", hub_build(), 'name = "runtime_aarch64"'),
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
