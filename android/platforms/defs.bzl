"""Coherent Rust and NDK target definitions for Android native builds."""

load("//android/sdk:repos.bzl", "ANDROID_API_LEVEL", "ANDROID_NDK_RELEASE")

DEVICE_TUPLE = "device"

EMULATOR_TUPLE = "emulator"

_ANDROID_OS = "@platforms//os:android"

_TUPLES = {
    "device": struct(
        cpu = "@platforms//cpu:arm64",
        rust_triple = "aarch64-linux-android",
    ),
    "emulator": struct(
        cpu = "@platforms//cpu:x86_64",
        rust_triple = "x86_64-linux-android",
    ),
}

_CXX_RUNTIMES = ["shared", "static", "none"]

_NDK_PREBUILT = "toolchains/llvm/prebuilt/linux-x86_64"

def tuples():
    """Returns the supported native tuple names."""
    return sorted(_TUPLES.keys())

def triple_for(name):
    """Returns the Rust target triple for one tuple name."""
    return _TUPLES[name].rust_triple if name in _TUPLES else ""

def cpu_for(name):
    """Returns the platform cpu constraint for one tuple name."""
    return _TUPLES[name].cpu if name in _TUPLES else ""

def constraint_list(name):
    """Returns the platform constraint values for one tuple name."""
    if name not in _TUPLES:
        return []
    return [_ANDROID_OS, _TUPLES[name].cpu]

def platform_label(name):
    """Returns the platform label for one tuple name."""
    return "//android/platforms:android_" + name

def rust_triples():
    """Returns the Rust target triples covering every tuple."""
    return sorted([info.rust_triple for info in _TUPLES.values()])

def api_floor():
    """Returns the minimum supported Android API level."""
    return ANDROID_API_LEVEL

def effective_api_level(api_level):
    """Returns the requested API level with zero meaning the floor."""
    return api_level if api_level != 0 else ANDROID_API_LEVEL

def clang_target(name, api_level):
    """Returns the NDK clang target prefix for one tuple and API level."""
    if name not in _TUPLES:
        return ""
    return _TUPLES[name].rust_triple + str(effective_api_level(api_level))

def ndk_prebuilt_dir():
    """Returns the NDK prebuilt host directory for the qualified host."""
    return _NDK_PREBUILT

def sysroot_api_dir(triple, api_level):
    """Returns the sysroot library directory for one triple and API level."""
    return "sysroot/usr/lib/" + triple + "/" + str(effective_api_level(api_level))

def tuple_errors(name, api_level):
    """Returns one error string per rejected tuple or API level selection."""
    if name not in _TUPLES:
        return ["android: unknown tuple '" + name + "': want " + ", ".join(tuples())]
    if api_level != 0 and api_level < ANDROID_API_LEVEL:
        return ["android: api_level " + str(api_level) + " is below " + str(ANDROID_API_LEVEL)]
    return []

def cxx_runtime_errors(runtime):
    """Returns one error string when the C++ runtime choice is unknown."""
    if runtime in _CXX_RUNTIMES:
        return []
    return ["android: cxx_runtime '" + runtime + "' is unknown: want " + ", ".join(_CXX_RUNTIMES)]

def ndk_revision_errors(revision):
    """Returns one error string when the NDK revision leaves the pinned lineage."""
    if revision.startswith(ANDROID_NDK_RELEASE.replace("r", "") + "."):
        return []
    return ["android: ndk revision '" + revision + "' is outside " + ANDROID_NDK_RELEASE]

def default_library_config(name):
    """Returns the default native library configuration for one tuple."""
    return struct(
        api_level = ANDROID_API_LEVEL,
        copts = [],
        cxx_runtime = "shared",
        features = [],
        rustc_flags = [],
        tuple = name,
    )

def library_config_errors(config):
    """Returns one error string per rejected field of one library configuration."""
    errors = list(tuple_errors(config.tuple, config.api_level))
    errors.extend(cxx_runtime_errors(config.cxx_runtime))
    return errors
