"""The rules_dotnet extensions and repository rules."""

load("@rules_dotnet//dotnet/private/rules/nuget:nuget_repo.bzl", _nuget_repo = "nuget_repo")
load("@rules_dotnet//dotnet:extensions.bzl", _dotnet = "dotnet")

dotnet = _dotnet
dotnet_nuget_repo = _nuget_repo
