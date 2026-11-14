"""The windows_support extensions over the patched archive that rules_dx fetches."""

load("@dx_windows_support//windows:extensions.bzl", _msvc_runtime = "msvc_runtime", _windows_sdk = "windows_sdk")

msvc_runtime = _msvc_runtime
windows_sdk = _windows_sdk
