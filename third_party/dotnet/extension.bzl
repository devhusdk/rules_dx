"""Creates the paket NuGet repositories for every group and the SDK packs."""

load("@rules_dotnet//dotnet:paket.paket2bazel_dependencies.bzl", "paket2bazel_dependencies")
load("@rules_dotnet//dotnet:paket.rules_dotnet_nuget_packages.bzl", "rules_dotnet_nuget_packages")
load("@rules_dotnet//dotnet/private/sdk/apphost_packs:dotnet.apphost_packs.bzl", "apphost_packs")
load("@rules_dotnet//dotnet/private/sdk/runtime_packs:dotnet.runtime_packs.bzl", "runtime_packs")
load("@rules_dotnet//dotnet/private/sdk/targeting_packs:dotnet.targeting_packs.bzl", "targeting_packs")
load("//third_party/dotnet/deps:paket.docs_csharp.bzl", "docs_csharp")
load("//third_party/dotnet/deps:paket.docs_fsharp.bzl", "docs_fsharp")
load("//third_party/dotnet/deps:paket.fsharplint.bzl", "fsharplint")
load("//third_party/dotnet/deps:paket.main.bzl", "main")
load("//third_party/dotnet/deps:paket.referencetrimmer.bzl", "referencetrimmer")

def _paket_impl(module_ctx):
    paket2bazel_dependencies()
    rules_dotnet_nuget_packages()
    targeting_packs()
    runtime_packs()
    apphost_packs()
    main()
    fsharplint()
    docs_csharp()
    docs_fsharp()
    referencetrimmer()
    return module_ctx.extension_metadata(reproducible = True)

paket = module_extension(
    implementation = _paket_impl,
)
