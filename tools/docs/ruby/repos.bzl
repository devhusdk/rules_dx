"""Pinned upstream YARD acquisition for Ruby API reference shards."""

load("//libs/starlark:repo.bzl", "gem_extract")

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
    gem_extract(
        ctx,
        url = ctx.attr.url,
        sha256 = ctx.attr.sha256,
        canonical_id = "dx-yard-gem:" + ctx.attr.url,
        output = ".",
        required = ["lib/yard.rb"],
    )
    ctx.file("BUILD.bazel", _LIB_BUILD)
    return ctx.repo_metadata(reproducible = True)

yard_repo = repository_rule(
    implementation = _yard_repo_impl,
    attrs = {
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)
