"""Public MDX API."""

load(
    "//mdx/private:defs.bzl",
    _mdx_library = "mdx_library",
    _mdx_build = "mdx_build",
    _mdx_test = "mdx_test",
)

mdx_library = _mdx_library
mdx_build = _mdx_build
mdx_test = _mdx_test
