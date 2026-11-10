"""Public Android API."""

load(
    "//android/private/packaging:defs.bzl",
    _abi_transition_settings = "abi_transition_settings",
    _abis_for_platforms = "abis_for_platforms",
    _android_adb_install = "android_adb_install",
    _android_adb_log = "android_adb_log",
    _android_adb_start = "android_adb_start",
    _android_apk_set = "android_apk_set",
    _android_check = "android_check",
    _android_sign = "android_sign",
    _rust_android_apk = "rust_android_apk",
)
load(
    "//android/private/rules:defs.bzl",
    _android_native_library = "android_native_library",
    _java_android_library = "java_android_library",
    _kotlin_android_library = "kotlin_android_library",
)

abi_transition_settings = _abi_transition_settings
abis_for_platforms = _abis_for_platforms
android_adb_install = _android_adb_install
android_adb_log = _android_adb_log
android_adb_start = _android_adb_start
android_apk_set = _android_apk_set
android_check = _android_check
android_native_library = _android_native_library
android_sign = _android_sign
java_android_library = _java_android_library
kotlin_android_library = _kotlin_android_library
rust_android_apk = _rust_android_apk
