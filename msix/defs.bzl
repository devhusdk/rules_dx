"""Public MSIX API."""

load(
    "//msix/private:defs.bzl",
    _MsixInfo = "MsixInfo",
    _msix_package = "msix_package",
    _msix_sign = "msix_sign",
)

MsixInfo = _MsixInfo
msix_package = _msix_package
msix_sign = _msix_sign
