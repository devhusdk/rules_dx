"""Pinned rules_dotnet and libfuzzer-dotnet archives; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_dotnet"]["archives"]

RULES_DOTNET_SHA256 = _ARCHIVES["rules_dotnet"]["sha256"]

RULES_DOTNET_STRIP_PREFIX = _ARCHIVES["rules_dotnet"]["strip_prefix"]

RULES_DOTNET_URL = _ARCHIVES["rules_dotnet"]["urls"][0]

LIBFUZZER_DOTNET_COMMIT = _ARCHIVES["libfuzzer_dotnet"]["version"]

LIBFUZZER_DOTNET_SHA256 = _ARCHIVES["libfuzzer_dotnet"]["sha256"]

LIBFUZZER_DOTNET_STRIP_PREFIX = _ARCHIVES["libfuzzer_dotnet"]["strip_prefix"]

LIBFUZZER_DOTNET_URL = _ARCHIVES["libfuzzer_dotnet"]["urls"][0]
