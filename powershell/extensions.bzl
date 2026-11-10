"""Public PowerShell module extension and repository rule."""

load("//powershell/private/toolchain:extensions.bzl", _powershell = "powershell")
load("//powershell/private/toolchain:gallery.bzl", _psgallery_module = "psgallery_module")

powershell = _powershell
psgallery_module = _psgallery_module
