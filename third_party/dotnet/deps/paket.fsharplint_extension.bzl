"Generated"

load(":paket.fsharplint.bzl", _fsharplint = "fsharplint")

def _fsharplint_impl(module_ctx):
    _fsharplint()
    return module_ctx.extension_metadata(reproducible = True)

fsharplint_extension = module_extension(
    implementation = _fsharplint_impl,
)
