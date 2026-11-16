"""Module extension for tools/docs/ruby."""

load(":repos.bzl", "yard_repo")

_REPOSITORY_NAME = "dx_yard"

def _yard_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("yard: the extension is only reachable from a module graph")
    pin = json.decode(module_ctx.read(Label(":pins.json")))["archives"]["yard"]
    yard_repo(
        name = _REPOSITORY_NAME,
        sha256 = pin["sha256"],
        urls = pin["urls"],
    )
    return module_ctx.extension_metadata(reproducible = True)

yard = module_extension(
    doc = "Downloads the YARD gem pinned in tools/docs/ruby/pins.json as @dx_yard.",
    implementation = _yard_impl,
)
