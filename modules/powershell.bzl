"""Pinned PowerShell foundation."""

load(":versions.bzl", _PWSH_VERSION = "PWSH_VERSION", _RULES_POWERSHELL_VERSION = "RULES_POWERSHELL_VERSION")

RULES_POWERSHELL_VERSION = _RULES_POWERSHELL_VERSION
PWSH_VERSION = _PWSH_VERSION

GALLERY_LOCK = "//third_party/powershell:PSGallery.lock.json"
GALLERY_REQUIREMENTS = "//third_party/powershell:PSGallery.requirements.psd1"

GALLERY_MEMBERS = [
    "Pester 5.7.1",
    "PSScriptAnalyzer 1.25.0",
]
