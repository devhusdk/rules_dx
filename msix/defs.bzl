"""Public MSIX API."""

load(
    "//msix/private:defs.bzl",
    _MsixInfo = "MsixInfo",
    _compatible_with = "compatible_with",
    _msix_package = "msix_package",
    _msix_sign = "msix_sign",
    _publisher_cn = "publisher_cn",
)

MsixInfo = _MsixInfo
compatible_with = _compatible_with
msix_package = _msix_package
msix_sign = _msix_sign
publisher_cn = _publisher_cn
