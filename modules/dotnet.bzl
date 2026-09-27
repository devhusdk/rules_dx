"""Pinned .NET foundation (C#, F#)."""

load(":versions.bzl", _BAZEL_LIB_VERSION = "BAZEL_LIB_VERSION", _DOTNET_VERSION = "DOTNET_VERSION", _RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION")

RULES_DOTNET_VERSION = _RULES_DOTNET_VERSION
BAZEL_LIB_VERSION = _BAZEL_LIB_VERSION
DOTNET_VERSION = _DOTNET_VERSION

PAKET_DEPENDENCIES = "//third_party/dotnet/paket.dependencies"
PAKET_LOCK = "//third_party/dotnet/paket.lock"
PAKET_REPIN = "bazel run @rules_dotnet//tools/paket2bazel -- --dependencies-file $PWD/third_party/dotnet/paket.dependencies --output-folder $PWD/third_party/dotnet/deps"

PAKET_MEMBERS = [
    "FSharp.Core 10.1.201",
    "xunit.v3 4.0.0",
    "xunit.analyzers 2.0.0",
]
