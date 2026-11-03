"""NuGet repository rule."""

load("@rules_dotnet//dotnet:defs.bzl", _nuget_repo = "nuget_repo")

nuget_repo = _nuget_repo
