"""Public Apple API."""

load("//apple/private/ios:app.bzl", _apple_ios_app = "apple_ios_app", _apple_ios_app_test = "apple_ios_app_test", _apple_ios_xcframework = "apple_ios_xcframework")
load("//apple/private/ios:cell.bzl", _APPLE_HOSTS = "APPLE_HOSTS")
load("//apple/private/ios:defs.bzl", _apple_ios_lib = "apple_ios_lib")
load("//apple/private/ios:device.bzl", _apple_ios_export = "apple_ios_export")
load("//apple/private/ios:simulator.bzl", _apple_simulator_boot = "apple_simulator_boot", _apple_simulator_install = "apple_simulator_install", _apple_simulator_launch = "apple_simulator_launch")
load("//apple/private/macos:app.bzl", _apple_macos_app = "apple_macos_app")
load("//apple/private/provisioning:defs.bzl", _apple_provisioning = "apple_provisioning")
load("//apple/private/provisioning:probe.bzl", _apple_toolchain_probe = "apple_toolchain_probe")
load("//apple/private/signing:defs.bzl", _apple_macos_notarize = "apple_macos_notarize", _apple_macos_sign = "apple_macos_sign")

APPLE_HOSTS = _APPLE_HOSTS
apple_ios_app = _apple_ios_app
apple_ios_app_test = _apple_ios_app_test
apple_ios_export = _apple_ios_export
apple_ios_lib = _apple_ios_lib
apple_ios_xcframework = _apple_ios_xcframework
apple_macos_app = _apple_macos_app
apple_macos_notarize = _apple_macos_notarize
apple_macos_sign = _apple_macos_sign
apple_provisioning = _apple_provisioning
apple_simulator_boot = _apple_simulator_boot
apple_simulator_install = _apple_simulator_install
apple_simulator_launch = _apple_simulator_launch
apple_toolchain_probe = _apple_toolchain_probe
