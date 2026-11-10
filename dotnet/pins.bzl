"""Pinned rules_dotnet and libfuzzer-dotnet archives."""

load("//core/modules:versions.bzl", "RULES_DOTNET_VERSION")

RULES_DOTNET_SHA256 = "ce35dad18db31cd68323b568dc09c689cc6d75ec16eba1510ccd128bb5f1264e"

RULES_DOTNET_STRIP_PREFIX = "rules_dotnet-" + RULES_DOTNET_VERSION

RULES_DOTNET_URL = "https://github.com/bazel-contrib/rules_dotnet/releases/download/v" + RULES_DOTNET_VERSION + "/rules_dotnet-v" + RULES_DOTNET_VERSION + ".tar.gz"

LIBFUZZER_DOTNET_COMMIT = "bd39d4e88d715ab460a929943645be2a186cde52"

LIBFUZZER_DOTNET_SHA256 = "c7e8281f29072c9a067d703aeeaaf8aa6b7b063cba2d3bd67cb8d85a48fa7c94"

LIBFUZZER_DOTNET_STRIP_PREFIX = "libfuzzer-dotnet-" + LIBFUZZER_DOTNET_COMMIT

LIBFUZZER_DOTNET_URL = "https://github.com/Metalnem/libfuzzer-dotnet/archive/" + LIBFUZZER_DOTNET_COMMIT + ".tar.gz"
