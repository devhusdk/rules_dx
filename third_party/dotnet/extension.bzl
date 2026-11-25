"""Re-exports the rules_dotnet paket extension, which MODULE.bazel cannot load from an archive repository."""

load("@rules_dotnet//dotnet:paket.bzl", _paket = "paket")

paket = _paket
