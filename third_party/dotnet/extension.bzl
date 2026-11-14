"""Creates the SDK packs and the rules_dotnet tool repositories."""

load("@rules_dotnet//dotnet:paket.paket2bazel_dependencies.bzl", "paket2bazel_dependencies")
load("@rules_dotnet//dotnet:paket.rules_dotnet_nuget_packages.bzl", "rules_dotnet_nuget_packages")
load("@rules_dotnet//dotnet/private/sdk/apphost_packs:dotnet.apphost_packs.bzl", "apphost_packs")
load("@rules_dotnet//dotnet/private/sdk/runtime_packs:dotnet.runtime_packs.bzl", "runtime_packs")
load("@rules_dotnet//dotnet/private/sdk/targeting_packs:dotnet.targeting_packs.bzl", "targeting_packs")

def _paket_impl(module_ctx):
    paket2bazel_dependencies()
    rules_dotnet_nuget_packages()
    targeting_packs()
    runtime_packs()
    apphost_packs()
    return module_ctx.extension_metadata(reproducible = True)

paket = module_extension(
    implementation = _paket_impl,
)
