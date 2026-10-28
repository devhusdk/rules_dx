"""paket.docs_csharp_extension helpers."""

"Generated"

load(":paket.docs_csharp.bzl", _docs_csharp = "docs_csharp")

def _docs_csharp_impl(module_ctx):
    _docs_csharp()
    return module_ctx.extension_metadata(reproducible = True)

docs_csharp_extension = module_extension(
    implementation = _docs_csharp_impl,
)
