"""Pinned upstream Doxygen acquisition for C, C++ and CUDA API reference shards."""

load("//libs/starlark:host.bzl", "host_cell")

DOXYGEN_VERSION = "1.18.0"

_RELEASE = "https://github.com/doxygen/doxygen/releases/download/Release_1_18_0/"

# One pinned official binary per host cell. The executable is the path inside the extracted archive.
DOXYGEN_CELLS = {
    "linux_x86_64": struct(
        archive = "doxygen-1.18.0.linux.bin.tar.gz",
        executable = "bin/doxygen",
        sha256 = "14fa81bdc34171edb5f1f02b1d60e74802f0439b77fa44e592565d517d72df90",
        strip_prefix = "doxygen-1.18.0",
    ),
    "macos_arm64": struct(
        archive = "doxygen-1.18.0-mac-arm.zip",
        executable = "doxygen",
        sha256 = "31d1c74467a9f6f456b4e6a4eff0c03e93d1ffa92da5fdf2d53a2f14ebaabbd7",
        strip_prefix = "doxygen-1.18.0",
    ),
    "macos_x86_64": struct(
        archive = "doxygen-1.18.0-mac-intel.zip",
        executable = "doxygen",
        sha256 = "8045d72f6f900fd430042339c1493d6eefcc85a7f990361fb57685682f16a2f9",
        strip_prefix = "doxygen-1.18.0",
    ),
    "windows_x86_64": struct(
        archive = "doxygen-1.18.0.windows.x64.bin.zip",
        executable = "doxygen.exe",
        sha256 = "e84f54cfd49ef06b0b16536056dbec0c496323de28abcce53a4269463de35eaf",
        strip_prefix = "",
    ),
}

# Upstream publishes no Linux arm64 or Windows arm64 build, and dx runs no host tool in its place.
UNSUPPORTED_REASON = "Doxygen publishes no official Linux arm64 or Windows arm64 build, so C, C++ and CUDA API docs are not generated on those hosts"

UNSUPPORTED_MARKER = "doxygen_unsupported.txt"

_REPOSITORY_NAME = "dx_doxygen"

# Only the binary and its DLL feed an action; the release also ships thousands of manual pages.
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
"""

def _doxygen_repo_impl(ctx):
    cell = DOXYGEN_CELLS.get(host_cell(ctx))
    if cell == None:
        ctx.file(UNSUPPORTED_MARKER, UNSUPPORTED_REASON + "\n")
    else:
        ctx.download_and_extract(
            url = _RELEASE + cell.archive,
            sha256 = cell.sha256,
            stripPrefix = cell.strip_prefix,
            canonical_id = "dx-doxygen:" + cell.archive,
        )
        if not ctx.path(cell.executable).exists:
            fail("dx_doxygen: " + cell.archive + " does not ship " + cell.executable)
    ctx.file("BUILD.bazel", _BUILD)
    return ctx.repo_metadata(reproducible = True)

_doxygen_repo = repository_rule(
    implementation = _doxygen_repo_impl,
)

def _doxygen_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("doxygen: the extension is only reachable from a module graph")
    _doxygen_repo(name = _REPOSITORY_NAME)
    return module_ctx.extension_metadata(reproducible = True)

doxygen = module_extension(
    doc = "Downloads the pinned, checksum-verified Doxygen release for the host as @dx_doxygen.",
    implementation = _doxygen_impl,
)
