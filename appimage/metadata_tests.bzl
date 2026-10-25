"""AppImage pin, host and name-rule coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "data_path", "name_errors")
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
load(":repos.bzl", "host_arch", "host_error", "hub_build", "unsupported_build")

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

def _data_path_checks():
    root = struct(repo_name = "", package = "")
    pkg = struct(repo_name = "", package = "pkg")
    ext = struct(repo_name = "repo", package = "pkg")
    ext_root = struct(repo_name = "repo", package = "")
    other = struct(repo_name = "", package = "other")
    sub = struct(repo_name = "", package = "sub")
    pkg_sub = struct(repo_name = "", package = "pkg/sub")
    return [
        expect_equal("root data keeps its subdirectory", data_path(struct(short_path = "assets/logo.png", basename = "logo.png", owner = root), root), "assets/logo.png"),
        expect_equal("root data a", data_path(struct(short_path = "a/x.txt", basename = "x.txt", owner = root), root), "a/x.txt"),
        expect_equal("root data b", data_path(struct(short_path = "b/x.txt", basename = "x.txt", owner = root), root), "b/x.txt"),
        expect_equal("subpackage data", data_path(struct(short_path = "pkg/assets/x", basename = "x", owner = pkg), pkg), "assets/x"),
        expect_equal("other package data", data_path(struct(short_path = "other/x", basename = "x", owner = other), pkg), "x"),
        expect_equal("external data in a root target", data_path(struct(short_path = "../repo/x", basename = "x", owner = ext_root), root), "x"),
        expect_equal("external package data", data_path(struct(short_path = "../repo/pkg/a/x", basename = "x", owner = ext), ext), "a/x"),
        expect_equal("external root data", data_path(struct(short_path = "../repo/a/x", basename = "x", owner = ext_root), ext_root), "a/x"),
        expect_equal("root target with other package data", data_path(struct(short_path = "other/x.txt", basename = "x.txt", owner = other), root), "x.txt"),
        expect_equal("root target with subpackage data", data_path(struct(short_path = "sub/x", basename = "x", owner = sub), root), "x"),
        expect_equal("package target with subpackage data", data_path(struct(short_path = "pkg/sub/x", basename = "x", owner = pkg_sub), pkg), "x"),
        expect_equal("generated data keeps its subdirectory", data_path(struct(short_path = "pkg/gen/x", basename = "x", owner = pkg), pkg), "gen/x"),
        expect_equal("data without an owner", data_path(struct(short_path = "pkg/a/x", basename = "x", owner = None), pkg), "x"),
    ]

def _checks():
    return _pin_checks() + _data_path_checks() + [
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
        expect_contains("other hosts mark the tools incompatible", unsupported_build(), 'target_compatible_with = ["@platforms//:incompatible"]'),
        expect_contains("other hosts expose every hub target", unsupported_build(), '["tools", "runtime_x86_64", "runtime_aarch64"]'),
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
