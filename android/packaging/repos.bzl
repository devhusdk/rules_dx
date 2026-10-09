"""Qualified upstream SDK selection for Rust Android application packaging."""

load("//android/sdk:repos.bzl", "ANDROID_API_LEVEL", "BUILD_TOOLS", "REQUIRED_LICENSES", "SDK_PLATFORM")

APP_SDK_REPOSITORY = "androidsdk"

APP_SDK_ENVIRON = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]

APP_SDK_API_LEVEL = ANDROID_API_LEVEL

APP_SDK_PLATFORM = SDK_PLATFORM

APP_SDK_BUILD_TOOLS = BUILD_TOOLS

APP_SDK_LICENSES = REQUIRED_LICENSES

KNOWN_ABIS = ["arm64-v8a", "x86_64"]

TUPLE_ABIS = {
    "device": "arm64-v8a",
    "emulator": "x86_64",
}

ABI_PLATFORMS = {
    "arm64-v8a": "//android/platforms:android_device",
    "x86_64": "//android/platforms:android_emulator",
}

def abis_for(tuples):
    """Returns the APK ABI names for native tuple names."""
    return [TUPLE_ABIS[name] for name in tuples if name in TUPLE_ABIS]

def abi_errors(abis):
    """Returns one error string per unknown APK ABI name."""
    errors = []
    for abi in abis:
        if abi not in KNOWN_ABIS:
            errors.append("rust_android_apk: abi '" + abi + "' is unknown: want " + ", ".join(KNOWN_ABIS))
    if len(abis) != len(dict([(abi, True) for abi in abis])):
        errors.append("rust_android_apk: abis holds duplicates")
    return errors

def app_id_errors(app_id):
    """Returns one error string when the application ID is not a dotted Java package."""
    parts = app_id.split(".")
    if len(parts) < 2:
        return ["rust_android_apk: app_id '" + app_id + "' needs at least two dot-separated parts"]
    errors = []
    for part in parts:
        if part == "":
            errors.append("rust_android_apk: app_id '" + app_id + "' holds an empty part")
            continue
        first = part[0]
        if not (first.isalpha() or first == "_"):
            errors.append("rust_android_apk: app_id part '" + part + "' must start with a letter")
        for index in range(len(part)):
            character = part[index]
            if not (character.isalnum() or character == "_"):
                errors.append("rust_android_apk: app_id part '" + part + "' holds '" + character + "'")
                break
    return errors

def sdk_request_errors(path, accept_licenses):
    """Returns one error string per rejected application SDK selection."""
    errors = []
    if path == "":
        return errors
    missing = [name for name in APP_SDK_LICENSES if name not in accept_licenses]
    if len(missing) > 0:
        errors.append("android_app_sdk: accept " + ", ".join(missing) + " before packaging")
    return errors

def manifest_lib_name(apk_name):
    """Returns the native library name the packaged APK loads."""
    return apk_name
