"""AppImage pin, host and name-rule coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":validate.bzl", "appimage_name_errors")
load(
    ":pins.bzl",
    "RUNTIMES",
    "SUPPORTED_CELLS",
    "TOOLS",
    "TOOL_BINARIES",
)
load(":repos.bzl", "host_arch", "host_error", "hub_build", "unsupported_build")

def _pin_checks():
    checks = []
    for arch in ["aarch64", "x86_64"]:
        checks.extend([
            expect_contains(arch + " tool url names the arch", TOOLS[arch]["url"], arch),
            expect_contains(arch + " runtime url names the arch", RUNTIMES[arch]["url"], arch),
        ])
    checks.append(expect_true("arch digests differ", TOOLS["aarch64"]["sha256"] != TOOLS["x86_64"]["sha256"] and RUNTIMES["aarch64"]["sha256"] != RUNTIMES["x86_64"]["sha256"]))
    return checks

def _checks():
    return _pin_checks() + [
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
        expect_contains("other hosts mark the tools incompatible", unsupported_build(), 'target_compatible_with = ["@platforms//:incompatible"]'),
        expect_contains("other hosts expose every hub target", unsupported_build(), '["tools", "runtime_x86_64", "runtime_aarch64"]'),
        expect_contains("hub exposes the x86_64 runtime", hub_build(), 'name = "runtime_x86_64"'),
        expect_contains("hub exposes the aarch64 runtime", hub_build(), 'name = "runtime_aarch64"'),
        expect_equal("matching names", appimage_name_errors("app", "app.desktop", "app.png"), []),
        expect_equal("svg icon", appimage_name_errors("app", "app.desktop", "app.svg"), []),
        expect_true("wrong desktop name", len(appimage_name_errors("app", "other.desktop", "app.png")) == 1),
        expect_true("wrong icon name", len(appimage_name_errors("app", "app.desktop", "other.png")) == 1),
    ]

def appimage_metadata_tests(name):
    """Declares the AppImage metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
