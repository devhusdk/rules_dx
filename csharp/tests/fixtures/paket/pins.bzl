"""Paket lock wiring pins."""

load("//modules:versions.bzl", _DOTNET_VERSION = "DOTNET_VERSION", _RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION")

RULES_DOTNET_VERSION = _RULES_DOTNET_VERSION

DOTNET_SDK_VERSION = _DOTNET_VERSION

PAKET_FRAMEWORK = "net10.0"

PAKET_SOURCE = "https://api.nuget.org/v3/index.json"

FSHARP_CORE_VERSION = "10.1.201"
XUNIT_V3_VERSION = "4.0.0"
XUNIT_ANALYZERS_VERSION = "2.0.0"

PAKET_DEPENDENCIES = "//third_party/dotnet:paket.dependencies"
PAKET_LOCK = "//third_party/dotnet:paket.lock"
PAKET_HUB = "//third_party/dotnet/deps:paket.main.bzl"
PAKET_EXTENSION = "//third_party/dotnet/deps:paket.main_extension.bzl"
PAKET_HUB_REPO = "@paket.main"

PAKET_HASH_ATTR = "sha512"

PAKET_GENERATION = "consumes never writes"

PAKET_FIXTURE_FSHARP_HELLO = "//fsharp/tests/fixtures/hello:hello_lib"
PAKET_FIXTURE_CSHARP_XUNIT = "//csharp/tests/fixtures/xunit:greeter_test"
PAKET_FIXTURE_FSHARP_XUNIT = "//fsharp/tests/fixtures/xunit:greeter_test"

PAKET_REJECTED = "packages.lock.json rejected: central paket.lock plus sha512 only"

PAKET_CURRENCY_RECHECK = "2026-09-22"
