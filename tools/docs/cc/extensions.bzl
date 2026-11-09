"""Module extension for tools/docs/cc."""

load(":repos.bzl", "REPOSITORY_NAME", "doxygen_repo")

def _doxygen_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("doxygen: the extension is only reachable from a module graph")
    doxygen_repo(name = REPOSITORY_NAME)
    return module_ctx.extension_metadata(reproducible = True)

doxygen = module_extension(
    doc = "Downloads the pinned, checksum-verified Doxygen release for the host as @dx_doxygen.",
    implementation = _doxygen_impl,
)
