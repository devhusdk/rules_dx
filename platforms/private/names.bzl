"""The rules_dx platform names with their OS, CPU and owner."""

visibility(["//platforms/..."])

def _name(os, cpu, owner, constraints):
    return struct(os = os, cpu = cpu, owner = owner, constraints = constraints)

_LINUX = "@platforms//os:linux"
_MACOS = "@platforms//os:macos"
_WINDOWS = "@platforms//os:windows"
_IOS = "@platforms//os:ios"
_ANDROID = "@platforms//os:android"
_X86_64 = "@platforms//cpu:x86_64"
_ARM64 = "@platforms//cpu:arm64"
_WASM32 = "@platforms//cpu:wasm32"

PLATFORMS = {
    "android_arm64": _name("android", "arm64", "android", [_ANDROID, _ARM64]),
    "android_x86_64": _name("android", "x86_64", "android", [_ANDROID, _X86_64]),
    "ios_arm64": _name("ios", "arm64", "apple", [_IOS, _ARM64]),
    "ios_sim_arm64": _name("ios", "arm64", "apple", [_IOS, _ARM64]),
    "ios_sim_x86_64": _name("ios", "x86_64", "apple", [_IOS, _X86_64]),
    "linux_arm64": _name("linux", "arm64", "core", [_LINUX, _ARM64]),
    "linux_x86_64": _name("linux", "x86_64", "core", [_LINUX, _X86_64]),
    "macos_arm64": _name("macos", "arm64", "core", [_MACOS, _ARM64]),
    "macos_x86_64": _name("macos", "x86_64", "core", [_MACOS, _X86_64]),
    "wasip1_wasm32": _name("wasip1", "wasm32", "core", ["@platforms//os:wasi", _WASM32]),
    "web_wasm32": _name("web", "wasm32", "core", ["@platforms//os:none", _WASM32]),
    "windows_arm64": _name("windows", "arm64", "core", [_WINDOWS, _ARM64]),
    "windows_x86_64": _name("windows", "x86_64", "core", [_WINDOWS, _X86_64]),
}

OS_NAMES = ["android", "ios", "linux", "macos", "web", "windows"]

OS_CONSTRAINTS = {
    "android": [_ANDROID],
    "ios": [_IOS],
    "linux": [_LINUX],
    "macos": [_MACOS],
    "wasip1": ["@platforms//os:wasi"],
    "web": ["@platforms//os:none", _WASM32],
    "windows": [_WINDOWS],
}

OS_DEFAULTS = {
    "android": "android_arm64",
    "ios": "ios_arm64",
    "linux": "linux_x86_64",
    "macos": "macos_arm64",
    "windows": "windows_x86_64",
}

GROUPS = {
    "apple": ["ios", "macos"],
    "posix": ["android", "ios", "linux", "macos"],
    "wasm": ["wasip1", "web"],
}

STUB_MARKS = {
    "android": "needs_rules_dx_android",
    "apple": "needs_rules_dx_apple",
}

HOSTS = {
    ("linux", "aarch64"): "linux_arm64",
    ("linux", "amd64"): "linux_x86_64",
    ("linux", "x86_64"): "linux_x86_64",
    ("mac os x", "aarch64"): "macos_arm64",
    ("mac os x", "x86_64"): "macos_x86_64",
    ("windows", "aarch64"): "windows_arm64",
    ("windows", "amd64"): "windows_x86_64",
    ("windows", "arm64"): "windows_arm64",
}

def host_name(os_name, arch):
    """Returns the platform name of a host, or "" when rules_dx has no name for it."""
    os = os_name.lower()
    for prefix in ["mac os x", "windows", "linux"]:
        if os.startswith(prefix):
            return HOSTS.get((prefix, arch.lower()), "")
    return ""

def matches(entry, name):
    """Returns whether a platforms entry, a full or OS-only name, covers the platform name."""
    if entry == name:
        return True
    return entry in OS_NAMES and name in PLATFORMS and PLATFORMS[name].os == entry

def full_name(entry, platforms):
    """Returns the platform name an entry moves to; an OS-only entry takes its first listed full name, else its OS default."""
    if entry in PLATFORMS:
        return entry
    listed = [name for name in platforms if name in PLATFORMS and PLATFORMS[name].os == entry]
    if listed:
        return listed[0]
    return OS_DEFAULTS.get(entry) or [name for name in sorted(PLATFORMS) if PLATFORMS[name].os == entry][0]

def platforms_error(platforms, native):
    """Returns why a platforms list is invalid, or "" when it is valid."""
    seen = {}
    for entry in platforms:
        if entry in seen:
            return "platforms lists {} twice".format(entry)
        seen[entry] = True
        if entry in PLATFORMS:
            continue
        if entry not in OS_NAMES:
            return "unknown platform {}: use one of {}".format(entry, ", ".join(sorted(PLATFORMS.keys() + OS_NAMES)))
        if native:
            cpus = [name for name in sorted(PLATFORMS) if PLATFORMS[name].os == entry]
            return "{} needs a cpu: {}".format(entry, " or ".join(cpus))
    return ""
