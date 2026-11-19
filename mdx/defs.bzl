"""Public MDX API."""

load(
    "//mdx/private:defs.bzl",
    _mdx_library = "mdx_library",
    _mdx_build = "mdx_build",
    _mdx_test = "mdx_test",
)
load("//tools/docs/components:defs.bzl", _mdx_doc_shard = "mdx_doc_shard")

mdx_library = _mdx_library
mdx_build = _mdx_build
mdx_test = _mdx_test
mdx_doc_shard = _mdx_doc_shard
