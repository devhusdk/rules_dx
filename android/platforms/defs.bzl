"""Coherent Rust and NDK target definitions for Android native builds."""

load("//android/private/sdk:repos.bzl", "ANDROID_API_LEVEL")
load(":validate.bzl", "TUPLES")

DEVICE_TUPLE = "device"

EMULATOR_TUPLE = "emulator"

_ANDROID_OS = str(Label("@platforms//os:android"))

_NDK_PREBUILT = "toolchains/llvm/prebuilt/linux-x86_64"

def triple_for(name):
    """Returns the Rust target triple for one tuple name."""
    return TUPLES[name].rust_triple if name in TUPLES else ""

def cpu_for(name):
    """Returns the platform cpu constraint for one tuple name."""
    return TUPLES[name].cpu if name in TUPLES else ""

def constraint_list(name):
    """Returns the platform constraint values for one tuple name."""
    if name not in TUPLES:
        return []
    return [_ANDROID_OS, TUPLES[name].cpu]

def platform_label(name):
    """Returns the platform label for one tuple name."""
    return str(Label("//android/platforms:android_" + name))

def rust_triples():
    """Returns the Rust target triples covering every tuple."""
    return sorted([info.rust_triple for info in TUPLES.values()])

def api_floor():
    """Returns the minimum supported Android API level."""
    return ANDROID_API_LEVEL

def effective_api_level(api_level):
    """Returns the requested API level with zero meaning the floor."""
    return api_level if api_level != 0 else ANDROID_API_LEVEL

def clang_target(name, api_level):
    """Returns the NDK clang target prefix for one tuple and API level."""
    if name not in TUPLES:
        return ""
    return TUPLES[name].rust_triple + str(effective_api_level(api_level))

def ndk_prebuilt_dir():
    """Returns the NDK prebuilt host directory for the qualified host."""
    return _NDK_PREBUILT

def sysroot_api_dir(triple, api_level):
    """Returns the sysroot library directory for one triple and API level."""
    return "sysroot/usr/lib/" + triple + "/" + str(effective_api_level(api_level))

