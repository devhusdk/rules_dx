"""The rules_dotnet extensions."""

load("@rules_dotnet//dotnet:extensions.bzl", _dotnet = "dotnet")
load("@rules_dotnet//dotnet:paket.paket2bazel_dependencies_extension.bzl", _paket2bazel_dependencies_extension = "paket2bazel_dependencies_extension")
load("@rules_dotnet//dotnet:paket.rules_dotnet_nuget_packages_extension.bzl", _rules_dotnet_nuget_packages_extension = "rules_dotnet_nuget_packages_extension")
load("@rules_dotnet//dotnet/private/sdk/apphost_packs:dotnet.apphost_packs_extension.bzl", _apphost_packs_extension = "apphost_packs_extension")
load("@rules_dotnet//dotnet/private/sdk/runtime_packs:dotnet.runtime_packs_extension.bzl", _runtime_packs_extension = "runtime_packs_extension")
load("@rules_dotnet//dotnet/private/sdk/targeting_packs:dotnet.targeting_packs_extension.bzl", _targeting_packs_extension = "targeting_packs_extension")

dotnet = _dotnet
paket2bazel_dependencies_extension = _paket2bazel_dependencies_extension
rules_dotnet_nuget_packages_extension = _rules_dotnet_nuget_packages_extension
targeting_packs_extension = _targeting_packs_extension
runtime_packs_extension = _runtime_packs_extension
apphost_packs_extension = _apphost_packs_extension
