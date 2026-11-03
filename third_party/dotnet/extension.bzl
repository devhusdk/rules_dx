"""Creates the paket NuGet repositories for every group."""

load("//third_party/dotnet/deps:paket.docs_csharp.bzl", "docs_csharp")
load("//third_party/dotnet/deps:paket.docs_fsharp.bzl", "docs_fsharp")
load("//third_party/dotnet/deps:paket.fsharplint.bzl", "fsharplint")
load("//third_party/dotnet/deps:paket.main.bzl", "main")

def _paket_impl(module_ctx):
    main()
    fsharplint()
    docs_csharp()
    docs_fsharp()
    return module_ctx.extension_metadata(reproducible = True)

paket = module_extension(
    implementation = _paket_impl,
)
