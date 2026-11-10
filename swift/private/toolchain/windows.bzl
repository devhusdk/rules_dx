"""Registers the rules_swift Windows toolchain over the unpacked swift.org installer."""

load("@rules_swift//swift/toolchains:swift_toolchain.bzl", "swift_toolchain")

_WINDOWS = ["@platforms//os:windows"]
_VERSION = "6.4.1"

def swift_windows_toolchain(name, cpu):
    """Declares the Swift toolchain for one Windows cpu."""
    repo = "@dx_swift_windows_" + cpu
    root = Label(repo + "//:.swift-version").workspace_root
    tags = ["manual"]

    swift_toolchain(
        name = "swift_toolchain_windows_" + cpu,
        arch = cpu,
        env = {"Path": (root + "/runtime/usr/bin").replace("/", "\\")},
        os = "windows",
        parsed_version = _VERSION,
        sdkroot = root + "/platform/Developer/SDKs/Windows.sdk",
        swift_tools = repo + "//:tools",
        tags = tags,
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

    native.toolchain(
        name = name,
        exec_compatible_with = _WINDOWS + ["@platforms//cpu:" + cpu],
        target_compatible_with = _WINDOWS + ["@platforms//cpu:" + cpu],
        toolchain = ":swift_toolchain_windows_" + cpu,
        toolchain_type = "@rules_swift//toolchains:toolchain_type",
    )
