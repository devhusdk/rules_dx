"""The rules_dotnet extensions."""

load("@rules_dotnet//dotnet:extensions.bzl", _dotnet = "dotnet")
load("@rules_dotnet//dotnet:paket.paket2bazel_dependencies.bzl", "paket2bazel_dependencies")
load("@rules_dotnet//dotnet:paket.rules_dotnet_nuget_packages.bzl", "rules_dotnet_nuget_packages")
load("@rules_dotnet//dotnet/private/sdk/apphost_packs:dotnet.apphost_packs.bzl", "apphost_packs")
load("@rules_dotnet//dotnet/private/sdk/runtime_packs:dotnet.runtime_packs.bzl", "runtime_packs")
load("@rules_dotnet//dotnet/private/sdk/targeting_packs:dotnet.targeting_packs.bzl", "targeting_packs")

dotnet = _dotnet

def _paket_impl(module_ctx):
    paket2bazel_dependencies()
    rules_dotnet_nuget_packages()
    targeting_packs()
    runtime_packs()
    apphost_packs()
    return module_ctx.extension_metadata(reproducible = True)

paket = module_extension(
    implementation = _paket_impl,
    doc = "Creates the paket and SDK pack repositories rules_dotnet needs.",
)
