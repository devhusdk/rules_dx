load("//android/private/sdk:repos.bzl", "ANDROID_API_LEVEL", "ANDROID_NDK_RELEASE")

TUPLES = {
    "device": struct(
        cpu = str(Label("@platforms//cpu:arm64")),
        rust_triple = "aarch64-linux-android",
    ),
    "emulator": struct(
        cpu = str(Label("@platforms//cpu:x86_64")),
        rust_triple = "x86_64-linux-android",
    ),
}

_CXX_RUNTIMES = ["shared", "static", "none"]

def android_tuples():
    """Returns the supported native tuple names."""
    return sorted(TUPLES.keys())

def android_tuple_errors(name, api_level):
    """Returns one error string per rejected tuple or API level selection."""
    if name not in TUPLES:
        return ["android: unknown tuple '" + name + "': want " + ", ".join(android_tuples())]
    if api_level != 0 and api_level < ANDROID_API_LEVEL:
        return ["android: api_level " + str(api_level) + " is below " + str(ANDROID_API_LEVEL)]
    return []

def android_cxx_runtime_errors(runtime):
    """Returns one error string when the C++ runtime choice is unknown."""
    if runtime in _CXX_RUNTIMES:
        return []
    return ["android: cxx_runtime '" + runtime + "' is unknown: want " + ", ".join(_CXX_RUNTIMES)]

def android_ndk_revision_errors(revision):
    """Returns one error string when the NDK revision leaves the pinned lineage."""
    if revision.startswith(ANDROID_NDK_RELEASE.replace("r", "") + "."):
        return []
    return ["android: ndk revision '" + revision + "' is outside " + ANDROID_NDK_RELEASE]

def android_default_library_config(name):
    """Returns the default native library configuration for one tuple."""
    return struct(
        api_level = ANDROID_API_LEVEL,
        copts = [],
        cxx_runtime = "shared",
        features = [],
        rustc_flags = [],
        tuple = name,
    )

def android_library_config_errors(config):
    """Returns one error string per rejected field of one library configuration."""
    errors = list(android_tuple_errors(config.tuple, config.api_level))
    errors.extend(android_cxx_runtime_errors(config.cxx_runtime))
    return errors
