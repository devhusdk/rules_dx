"""C and C++ tool acquisition via a lazy module extension."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", _http_archive = "http_archive")
load("//quality/tools/cc:repos.bzl", "CC_TOOLS")

def _cc_tools_impl(_ctx):
    for name in sorted(CC_TOOLS.keys()):
        spec = CC_TOOLS[name]
        if spec["kind"] != "archive":
            fail("unsupported C/C++ tool kind for '" + name + "': " + spec["kind"])
        _http_archive(
            name = name,
            build_file = spec["build_file"],
            sha256 = spec["sha256"],
            strip_prefix = spec["strip_prefix"],
            urls = spec["urls"],
        )

cc_tools = module_extension(implementation = _cc_tools_impl)
