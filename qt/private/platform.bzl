"""The Qt platform an executable or bundle builds for."""

load("@platforms//host:constraints.bzl", "HOST_CONSTRAINTS")
load("//qt/private/toolchain:pins.bzl", "CELLS")
load("//qt/private/toolchain:repos.bzl", "qualified_cells")

_COMPILER = str(Label("//cc:compiler"))
_PLATFORMS = "//command_line_option:platforms"

_HOST_PLATFORMS = [
    str(Label("@bazel_tools//tools:host_platform")),
    str(Label("@platforms//host")),
]

_OS = {
    str(Label("@platforms//os:linux")): "linux",
    str(Label("@platforms//os:macos")): "macos",
    str(Label("@platforms//os:windows")): "windows",
}

_CPU = {
    str(Label("@platforms//cpu:aarch64")): "arm64",
    str(Label("@platforms//cpu:arm64")): "arm64",
    str(Label("@platforms//cpu:x86_64")): "x86_64",
}

QT_PLATFORMS = {cell: str(Label("//qt:" + cell)) for cell in qualified_cells()}

QT_MSVC_PLATFORM_FLAGS = ["--{}=msvc".format(_COMPILER)]

_PLATFORM_COMPILERS = {QT_PLATFORMS[cell]: CELLS[cell].layout.compiler for cell in QT_PLATFORMS}

def qt_host_cell(constraints):
    """Returns the qualified Qt cell of a host with constraints, or ""."""
    os = ""
    cpu = ""
    for constraint in constraints:
        key = str(Label(constraint))
        os = _OS.get(key, os)
        cpu = _CPU.get(key, cpu)
    cell = os + "_" + cpu
    return cell if os and cpu and cell in QT_PLATFORMS else ""

_HOST_CELL = qt_host_cell(HOST_CONSTRAINTS)

def qt_platforms(attr, current):
    """Returns attr.platforms, the Qt platform of the host on a host build, or [] to keep current."""
    chosen = [str(p) for p in (getattr(attr, "platforms", None) or [])]
    if chosen:
        return chosen
    if _HOST_CELL and [str(p) for p in current] in [[host] for host in _HOST_PLATFORMS]:
        return [QT_PLATFORMS[_HOST_CELL]]
    return []

def qt_platform_settings(settings, platforms):
    """Returns the compiler a Qt platform in platforms builds with, or the current compiler."""
    compiler = _PLATFORM_COMPILERS.get(platforms[0], "") if platforms else ""
    return {_COMPILER: compiler or settings[_COMPILER]}

def _platform_transition_impl(settings, attr):
    platforms = qt_platforms(attr, settings[_PLATFORMS])
    if len(platforms) > 1:
        fail("qt: platforms takes exactly one platform label, got " + str(len(platforms)))
    out = qt_platform_settings(settings, platforms)
    out[_PLATFORMS] = platforms or settings[_PLATFORMS]
    return out

QT_PLATFORM_TRANSITION = transition(
    implementation = _platform_transition_impl,
    inputs = [_PLATFORMS, _COMPILER],
    outputs = [_PLATFORMS, _COMPILER],
)

QT_PLATFORMS_ATTR = {
    "platforms": attr.string_list(
        doc = "Platform label to build for. Defaults to the Qt platform of the host on a host build, else the current target platform.",
    ),
}
