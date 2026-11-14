"""Public C and C++ module extensions."""

load("@dx_windows_support//windows:extensions.bzl", _msvc_runtime = "msvc_runtime", _windows_sdk = "windows_sdk")
load("//cc/private/toolchain:extensions.bzl", _gcc = "gcc")
load("//cc/private/toolchain/msvc:extensions.bzl", _msvc = "msvc")

gcc = _gcc
msvc = _msvc
msvc_runtime = _msvc_runtime
windows_sdk = _windows_sdk
