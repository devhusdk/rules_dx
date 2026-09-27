"""Gallery lock wiring pins."""

load("//modules:versions.bzl", _PWSH_VERSION = "PWSH_VERSION", _RULES_POWERSHELL_VERSION = "RULES_POWERSHELL_VERSION")

RULES_POWERSHELL_VERSION = _RULES_POWERSHELL_VERSION

PWSH_VERSION = _PWSH_VERSION

PESTER_VERSION = "5.7.1"
PSSCRIPTANALYZER_VERSION = "1.25.0"

GALLERY_SOURCE = "https://www.powershellgallery.com/api/v2"

GALLERY_LOCK = "//third_party/powershell:PSGallery.lock.json"
GALLERY_REQUIREMENTS = "//third_party/powershell:PSGallery.requirements.psd1"

GALLERY_GENERATION = "consumes never writes"
GALLERY_FAIL_CLOSED = "fail_if_repin_required"

GALLERY_FIXTURE_HELLO = "//powershell/tests/fixtures/hello:hello_lib"
GALLERY_FIXTURE_PESTER = "//powershell/tests/fixtures/pester:greeter_test"

GALLERY_REJECTED = "Install-Module rejected: exact lock plus explicit-path import only"

GALLERY_CURRENCY_RECHECK = "2026-09-22"
