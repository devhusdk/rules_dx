"""Doxygen acquisition for C, C++ and CUDA API reference shards, pinned in tools/docs/cc/pins.json."""

load("//core:host.bzl", "host_cell")

UNSUPPORTED_REASON = "Doxygen publishes no official Linux arm64 or Windows arm64 build, so C, C++ and CUDA API docs are not generated on those hosts"

UNSUPPORTED_MARKER = "doxygen_unsupported.txt"

REPOSITORY_NAME = "dx_doxygen"

_BUILD = """\
filegroup(
    name = "tool",
    srcs = glob([
        "bin/doxygen",
        "doxygen",
        "doxygen.exe",
        "libclang.dll",
        "doxygen_unsupported.txt",
    ], allow_empty = True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "binary_or_marker",
    srcs = ["{binary}"],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "binary",
    srcs = ["{binary}"],
    target_compatible_with = {compatible_with},
    visibility = ["//visibility:public"],
)
"""

def _doxygen_repo_impl(ctx):
    cell = json.decode(ctx.read(Label(":pins.json")))["cells"].get(host_cell(ctx))
    binary = UNSUPPORTED_MARKER
    compatible_with = ["@platforms//:incompatible"]
    if cell == None:
        ctx.file(UNSUPPORTED_MARKER, UNSUPPORTED_REASON + "\n")
    else:
        ctx.download_and_extract(
            url = cell["urls"],
            sha256 = cell["sha256"],
            stripPrefix = cell.get("strip_prefix", ""),
            canonical_id = "dx-doxygen:" + cell["urls"][0],
        )
        if not ctx.path(cell["binary"]).exists:
            fail("dx_doxygen: " + cell["urls"][0] + " does not ship " + cell["binary"])
        binary = cell["binary"]
        compatible_with = []
    ctx.file("BUILD.bazel", _BUILD.format(binary = binary, compatible_with = repr(compatible_with)))
    return ctx.repo_metadata(reproducible = True)

doxygen_repo = repository_rule(
    implementation = _doxygen_repo_impl,
)
