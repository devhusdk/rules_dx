"""Pinned upstream YARD acquisition for Ruby API reference shards."""

YARD_VERSION = "0.9.45"

YARD_SHA256 = "52e211493f7cb8a3ebf7e104a25a1e73937a3103092545d34cb88fafebb3dc51"

YARD_URL = "https://rubygems.org/downloads/yard-" + YARD_VERSION + ".gem"

_REPOSITORY_NAME = "dx_yard"

_LIB_BUILD = """\
filegroup(
    name = "lib",
    srcs = glob(["lib/**"]),
    visibility = ["//visibility:public"],
)

exports_files([
    "LICENSE",
    "lib/yard.rb",
])
"""

def _yard_repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.url,
        output = "gem",
        sha256 = ctx.attr.sha256,
        type = "tar",
        canonical_id = "dx-yard-gem:" + ctx.attr.url,
    )
    data = "gem/data.tar.gz"
    if not ctx.path(data).exists:
        fail("yard: '" + ctx.attr.url + "' holds no data.tar.gz")
    ctx.extract(archive = data, output = ".")
    if not ctx.path("lib/yard.rb").exists:
        fail("yard: '" + ctx.attr.url + "' holds no lib/yard.rb")
    ctx.delete("gem")
    ctx.file("BUILD.bazel", _LIB_BUILD)
    return ctx.repo_metadata(reproducible = True)

_yard_repo = repository_rule(
    implementation = _yard_repo_impl,
    attrs = {
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)

def _yard_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("yard: the extension is only reachable from a module graph")
    _yard_repo(
        name = _REPOSITORY_NAME,
        sha256 = YARD_SHA256,
        url = YARD_URL,
    )
    return module_ctx.extension_metadata(reproducible = True)

yard = module_extension(
    doc = "Downloads the pinned, checksum-verified YARD gem as @dx_yard.",
    implementation = _yard_impl,
)
