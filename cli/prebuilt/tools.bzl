"""The rules_dx tools a release ships prebuilt, and how a consumer finds each one.

Hand-written. fill_pins reads the asset list rendered from this file
(//cli/prebuilt:registry) and writes one sha256 per tool and platform into
pins.bzl, so adding a tool here and to release.yml is all a new prebuilt needs.

A platform key is "<os>_<cpu>" in release asset spelling: os is linux, macos or
windows, cpu is x86_64 or arm64, and a jar that runs anywhere uses "any".
"""

visibility(["//cli/...", "//gazelle/..."])

# Release asset spelling to @platforms constraint values.
RELEASE_OS = {
    "linux": "linux",
    "macos": "macos",
    "windows": "windows",
}

RELEASE_CPU = {
    "arm64": "aarch64",
    "x86_64": "x86_64",
}

_NATIVE = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]

# Each tool:
#   asset: the release asset name; {os} and {cpu} are filled from the platform key.
#   role: "exec" when the tool runs during the build (the hub selects on the exec
#     platform), "target" when it ships inside the built output (target platform).
#   platforms: the platform keys a release builds.
#   fallbacks: a platform key with no pin of its own that may run another key's
#     download, such as windows_arm64 running x86_64 under emulation.
#   source: the label that builds the tool when there is no usable pin.
#   member: the file to run inside an archive asset; "" for a bare binary.
TOOLS = {
    "dx": struct(
        asset = "dx-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_dx//cli/cli:dx",
        member = "",
    ),
    "gazelle": struct(
        asset = "dx-gazelle-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        # The gazelle hub builds its own gazelle_binary from the registered languages.
        source = "@dx_gazelle//:gazelle_source",
        member = "",
    ),
}

def split_key(key):
    """Returns (os, cpu) in release spelling for a platform key such as "linux_x86_64"."""
    os, _, cpu = key.partition("_")
    return os, cpu

def asset_name(tool, key):
    """Returns the release asset name of tool on the platform key."""
    os, cpu = split_key(key)
    return TOOLS[tool].asset.format(os = os, cpu = cpu)

def registry_lines():
    """Returns one "<tool> <key> <asset>" line per release asset, sorted, for fill_pins."""
    return sorted([
        "{} {} {}".format(tool, key, asset_name(tool, key))
        for tool, entry in TOOLS.items()
        for key in entry.platforms
    ])
