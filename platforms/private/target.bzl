"""Platform providers, the transition that moves a target to its platform, and current_target."""

load("@dx_platforms//:host.bzl", "HOST")
load(":names.bzl", "PLATFORMS", "matches")

visibility(["//platforms/..."])

DxPlatformInfo = provider(
    doc = "The platform a target built for and the platforms it lists.",
    fields = {
        "name": "Platform name the target built for; empty means the host.",
        "platforms": "Platform names from the target's platforms attr; empty means any.",
    },
)

DxTargetInfo = provider(
    doc = "What a family's target toolchain adds for its platforms.",
    fields = {
        "check": "Executable run on library and binary outputs, or None.",
        "java_deps": "Neverlink JavaInfo for OS-only JVM targets.",
        "link_flags": "Link flags added to the CcInfo of static libraries.",
        "min_os_floor": "Dict from OS to the minimum version used when min_os is empty.",
        "runner": "Executable that runs binaries and tests for the platform, or None.",
    },
)

TARGET_TOOLCHAIN = Label("//platforms:target_toolchain_type")

_PLATFORMS = "//command_line_option:platforms"
_CURRENT = str(Label("//platforms:current"))

# Each OS that min_os accepts, and the flag that holds its minimum version.
MIN_OS_FLAGS = {
    "android": str(Label("//platforms:min_os_android")),
    "ios": "//command_line_option:ios_minimum_os",
    "macos": "//command_line_option:macos_minimum_os",
    "windows": str(Label("//platforms:min_os_windows")),
}

def current_name(current):
    """Returns the platform name a build is for, with the empty flag meaning the host."""
    return current or HOST

def move_to(platforms, current):
    """Returns the platform name a target with this platforms list moves to, or "" when it stays."""
    name = current_name(current)
    for entry in platforms:
        if name and matches(entry, name):
            return ""

    # An OS-only name has no platform of its own, so only a full name can move the target.
    for entry in platforms:
        if entry in PLATFORMS:
            return entry
    return ""

def min_os_error(min_os):
    """Returns why a min_os dict is invalid, or "" when it is valid."""
    for os in min_os:
        if os not in MIN_OS_FLAGS:
            return "min_os: unknown OS {}: use one of {}".format(os, ", ".join(sorted(MIN_OS_FLAGS)))
    return ""

def _transition_impl(settings, attr):
    out = {key: settings[key] for key in [_PLATFORMS, _CURRENT] + MIN_OS_FLAGS.values()}
    min_os = getattr(attr, "min_os", None) or {}
    error = min_os_error(min_os)
    if error:
        fail(error)
    for os, version in min_os.items():
        out[MIN_OS_FLAGS[os]] = version
    target = move_to(getattr(attr, "platforms", None) or [], settings[_CURRENT])
    if target:
        out[_PLATFORMS] = [str(Label("//platforms:" + target))]
        out[_CURRENT] = target
    return out

platform_transition = transition(
    implementation = _transition_impl,
    inputs = [_PLATFORMS, _CURRENT] + MIN_OS_FLAGS.values(),
    outputs = [_PLATFORMS, _CURRENT] + MIN_OS_FLAGS.values(),
)

_EMPTY = DxTargetInfo(check = None, java_deps = [], link_flags = [], min_os_floor = {}, runner = None)

def _current_target_impl(ctx):
    toolchain = ctx.toolchains[TARGET_TOOLCHAIN]
    if toolchain == None:
        return [_EMPTY]
    return [toolchain.dx_target]

current_target = rule(
    implementation = _current_target_impl,
    doc = "Returns the DxTargetInfo of the target toolchain for the current platform, or an empty one.",
    toolchains = [config_common.toolchain_type(TARGET_TOOLCHAIN, mandatory = False)],
)
