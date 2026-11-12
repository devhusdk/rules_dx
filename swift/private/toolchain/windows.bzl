"""Declares the rules_swift Windows toolchain over the unpacked swift.org installer."""

load("@rules_swift//swift/toolchains:swift_toolchain.bzl", "swift_toolchain")

_WINDOWS = ["@platforms//os:windows"]
_VERSION = "6.4.2"

def swift_windows_toolchain(name, cpu):
    """Declares the Swift toolchain for one Windows cpu."""
    repo = "@dx_swift_windows_" + cpu
    root = Label(repo + "//:.swift-version").workspace_root
    runtime = (root + "/runtime/usr/bin").replace("/", "\\")

    swift_toolchain(
        name = name,
        arch = cpu,
        env = {"PATH": runtime, "Path": runtime},
        os = "windows",
        parsed_version = _VERSION,
        sdkroot = root + "/platform/Developer/SDKs/Windows.sdk",
        swift_tools = repo + "//:tools",
        tags = ["manual"],
        target_compatible_with = _WINDOWS,
        tool_executable_suffix = ".exe",
        version_file = repo + "//:.swift-version",
        xctest_version = _VERSION,
        features = [
            "swift._supports_upcoming_features",
            "swift.no_embed_debug_module",
            "swift.module_map_no_private_headers",
        ],
    )
