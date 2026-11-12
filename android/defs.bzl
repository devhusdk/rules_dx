"""Public Android API."""

load(
    "//android/private/packaging:defs.bzl",
    _android_adb_install = "android_adb_install",
    _android_adb_log = "android_adb_log",
    _android_adb_start = "android_adb_start",
    _android_apk_set = "android_apk_set",
    _android_check = "android_check",
    _android_sign = "android_sign",
    _android_rust_apk = "android_rust_apk",
)
load(
    "//android/private/library:defs.bzl",
    _android_native_library = "android_native_library",
    _android_java_library = "android_java_library",
    _android_kotlin_library = "android_kotlin_library",
)

android_adb_install = _android_adb_install
android_adb_log = _android_adb_log
android_adb_start = _android_adb_start
android_apk_set = _android_apk_set
android_check = _android_check
android_native_library = _android_native_library
android_sign = _android_sign
android_java_library = _android_java_library
android_kotlin_library = _android_kotlin_library
android_rust_apk = _android_rust_apk
