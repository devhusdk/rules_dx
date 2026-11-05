"""Platform constraints and wheel tags for the native probe wheel."""

NATIVE_PROBE_COMPATIBLE = select({
    "@platforms//os:linux": ["@platforms//cpu:x86_64"],
    "@platforms//os:windows": ["@platforms//cpu:x86_64"],
    "//conditions:default": ["@platforms//:incompatible"],
})

NATIVE_PROBE_PLATFORM = select({
    "@platforms//os:windows": "win_amd64",
    "//conditions:default": "linux_x86_64",
})
