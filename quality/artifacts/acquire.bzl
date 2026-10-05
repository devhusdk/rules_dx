"""Standalone tool acquisition with Bazel download and extract only."""

ARCHIVE_TYPES = {
    "gzip": "gz",
    "none": "",
    "tar.gz": "tar.gz",
    "zip": "zip",
}

ACQUIRE_ATTRS = {
    "archive_format": attr.string(mandatory = True),
    "asset": attr.string(mandatory = True),
    "executable": attr.string(mandatory = True),
    "executable_sha256": attr.string(mandatory = True),
    "sha256": attr.string(mandatory = True),
    "url": attr.string(mandatory = True),
}

_STAGING = "dx_extracted"

def acquire_tool(ctx):
    """Fetches one tool, verifies its bytes and writes its BUILD file."""
    kind = ctx.attr.archive_format
    if kind not in ARCHIVE_TYPES:
        fail("standalone tool repo: unsupported archive format '" + kind +
             "'; want one of " + ", ".join(sorted(ARCHIVE_TYPES.keys())))
    if ctx.attr.executable_sha256 == "":
        fail("standalone tool repo: missing executable_sha256 for '" + ctx.attr.executable +
             "' (regenerate metadata with //quality/artifacts:update)")
    if kind == "none":
        _download_executable(ctx)
    else:
        _verify_executable(ctx, kind)
    ctx.file("BUILD.bazel", _tool_build(ctx))

def _download_executable(ctx):
    """Fetches the executable itself as one verified, executable file."""
    if ctx.attr.executable_sha256 != ctx.attr.sha256:
        fail("standalone tool repo: '" + ctx.attr.executable +
             "' has no archive, so executable_sha256 must equal sha256; got " +
             ctx.attr.executable_sha256 + " and " + ctx.attr.sha256 +
             " (regenerate metadata with //quality/artifacts:update)")
    ctx.download(
        url = ctx.attr.url,
        output = ctx.attr.executable,
        sha256 = ctx.attr.sha256,
        executable = True,
        canonical_id = "dx-tool:" + ctx.attr.url,
    )

def _verify_executable(ctx, kind):
    """Publishes the extracted executable under its recorded digest."""
    ctx.download_and_extract(
        url = ctx.attr.url,
        output = _STAGING,
        sha256 = ctx.attr.sha256,
        type = ARCHIVE_TYPES[kind],
        canonical_id = "dx-tool:" + ctx.attr.url,
    )
    ctx.download(
        url = _file_url(ctx, _extracted(ctx, kind)),
        output = ctx.attr.executable,
        sha256 = ctx.attr.executable_sha256,
        executable = True,
        canonical_id = "dx-tool-executable:" + ctx.attr.url + ":" + ctx.attr.executable_sha256,
    )

def _extracted(ctx, kind):
    """Returns the repository path one extraction wrote the executable to."""
    candidates = [_STAGING + "/" + ctx.attr.executable]
    if kind == "gzip":
        candidates.append(_STAGING + "/" + _gzip_member(ctx.attr.asset))
    for candidate in candidates:
        if ctx.path(candidate).exists:
            return candidate
    fail("standalone tool repo: " + kind + " extraction of '" + ctx.attr.asset +
         "' produced no '" + ctx.attr.executable + "'")

def _gzip_member(asset):
    """Returns the path Bazel names one gzip extraction after."""
    return asset[:-3] if asset.endswith(".gz") else asset

def _file_url(ctx, path):
    """Returns the file URL of one repository path."""
    text = str(ctx.path(path)).replace("\\", "/")
    return "file://" + text if text.startswith("/") else "file:///" + text

def _tool_build(ctx):
    """Returns the BUILD file exposing one acquired tool."""
    return "\n".join([
        "filegroup(",
        '    name = "tool",',
        "    srcs = [%r]," % ctx.attr.executable,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])
