"""Standalone tool acquisition with Bazel download and extract only."""

ARCHIVE_TYPES = {
    "gzip": "gz",
    "none": "",
    "tar.gz": "tar.gz",
    "tar.xz": "tar.xz",
    "zip": "zip",
    "zstd": "zst",
}

ACQUIRE_ATTRS = {
    "archive_format": attr.string(
        doc = "Archive format of the download, or none for a bare executable.",
        mandatory = True,
    ),
    "asset": attr.string(
        doc = "Upstream release asset name the tool was published as.",
        mandatory = True,
    ),
    "executable": attr.string(
        doc = "File name the tool executable is written to.",
        mandatory = True,
    ),
    "executable_sha256": attr.string(
        doc = "Expected sha256 of the extracted executable file.",
        mandatory = True,
    ),
    "runtime_files": attr.string(
        default = "[]",
        doc = "JSON list of extra files written next to the executable.",
    ),
    "sha256": attr.string(
        doc = "Expected sha256 of the downloaded archive or executable.",
        mandatory = True,
    ),
    "url": attr.string(
        doc = "Download URL of the upstream archive or executable.",
        mandatory = True,
    ),
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
             "' (regenerate metadata with //core/quality/artifacts:update)")
    if kind == "none":
        _download_executable(ctx)
    else:
        _verify_executable(ctx, kind)
    runtime = _runtime(ctx)
    ctx.file("BUILD.bazel", _tool_build(ctx, runtime))

def _download_executable(ctx):
    """Fetches the executable itself as one verified, executable file."""
    if ctx.attr.executable_sha256 != ctx.attr.sha256:
        fail("standalone tool repo: '" + ctx.attr.executable +
             "' has no archive, so executable_sha256 must equal sha256; got " +
             ctx.attr.executable_sha256 + " and " + ctx.attr.sha256 +
             " (regenerate metadata with //core/quality/artifacts:update)")
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

def _runtime(ctx):
    """Writes the runtime files and returns their paths."""
    paths = []
    staged = {}
    for entry in json.decode(ctx.attr.runtime_files):
        path = entry["path"]
        if "content" in entry:
            ctx.file(path, entry["content"])
        else:
            if entry["format"] not in ARCHIVE_TYPES or entry["format"] in ("none", "gzip"):
                fail("standalone tool repo: unsupported runtime archive format '" +
                     entry["format"] + "' for '" + path + "'")
            if entry["url"] not in staged:
                staging = "dx_runtime_%d" % len(staged)
                ctx.download_and_extract(
                    url = entry["url"],
                    output = staging,
                    sha256 = entry["sha256"],
                    type = ARCHIVE_TYPES[entry["format"]],
                    canonical_id = "dx-tool-runtime:" + entry["url"],
                )
                staged[entry["url"]] = staging
            ctx.download(
                url = _file_url(ctx, staged[entry["url"]] + "/" + entry["member"]),
                output = path,
                sha256 = entry["member_sha256"],
                canonical_id = "dx-tool-runtime-file:" + entry["url"] + ":" + entry["member_sha256"],
            )
        paths.append(path)
    return paths

def _extracted(ctx, kind):
    """Returns the repository path one extraction wrote the executable to."""
    candidates = [_STAGING + "/" + ctx.attr.executable]
    if kind in ("gzip", "zstd"):
        candidates.append(_STAGING + "/" + _compressed_member(ctx.attr.asset, kind))
    for candidate in candidates:
        if ctx.path(candidate).exists:
            return candidate
    fail("standalone tool repo: " + kind + " extraction of '" + ctx.attr.asset +
         "' produced no '" + ctx.attr.executable + "'")

def _compressed_member(asset, kind):
    """Returns the path Bazel names one single-file extraction after."""
    suffix = ".gz" if kind == "gzip" else ".zst"
    return asset[:-len(suffix)] if asset.endswith(suffix) else asset

def _file_url(ctx, path):
    """Returns the file URL of one repository path."""
    text = str(ctx.path(path)).replace("\\", "/")
    return "file://" + text if text.startswith("/") else "file:///" + text

def _tool_build(ctx, runtime):
    """Returns the BUILD file exposing one acquired tool."""
    return "\n".join([
        "filegroup(",
        '    name = "tool",',
        "    srcs = [%r]," % ctx.attr.executable,
        "    data = %r," % runtime,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])
