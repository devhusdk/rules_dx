"""Module extension for tools/docs/ruby."""

load(":repos.bzl", "yard_repo")


YARD_VERSION = "0.9.45"


YARD_SHA256 = "52e211493f7cb8a3ebf7e104a25a1e73937a3103092545d34cb88fafebb3dc51"


YARD_URL = "https://rubygems.org/downloads/yard-" + YARD_VERSION + ".gem"


_REPOSITORY_NAME = "dx_yard"


def _yard_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("yard: the extension is only reachable from a module graph")
    yard_repo(
        name = _REPOSITORY_NAME,
        sha256 = YARD_SHA256,
        url = YARD_URL,
    )
    return module_ctx.extension_metadata(reproducible = True)


yard = module_extension(
    doc = "Downloads the pinned, checksum-verified YARD gem as @dx_yard.",
    implementation = _yard_impl,
)
