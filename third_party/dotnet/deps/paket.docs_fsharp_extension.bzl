"""paket.docs_fsharp_extension helpers."""

"Generated"

load(":paket.docs_fsharp.bzl", _docs_fsharp = "docs_fsharp")

def _docs_fsharp_impl(module_ctx):
    _docs_fsharp()
    return module_ctx.extension_metadata(reproducible = True)

docs_fsharp_extension = module_extension(
    implementation = _docs_fsharp_impl,
)
