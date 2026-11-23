"""Shared repository rules that write rendered files and fetch pinned archives."""

visibility(["//android/...", "//apple/...", "//cc/...", "//cli/...", "//core/...", "//d/...", "//elixir/...", "//erlang/...", "//gazelle/...", "//haskell/...", "//julia/...", "//msix/...", "//ocaml/...", "//ruby/...", "//terraform/...", "//tools/...", "//typescript/..."])

ARCHIVE_TYPES = ["zip", "war", "aar", "tar", "tar.gz", "tgz", "tar.xz", "txz", "tar.zst", "tzst", "tar.bz2", "tbz", "ar", "deb", "7z"]

def _write_files(rctx):
    rctx.file("REPO.bazel", "")
    for path, content in rctx.attr.files.items():
        rctx.file(path, content)

_BZL_SRCS = """
filegroup(
    name = "dx_bzl_srcs",
    srcs = glob(["**/*.bzl", "**/*.scl"], allow_empty = True) + {packages},
    visibility = ["//visibility:public"],
)
"""

def _build_file(dir):
    for name in ("BUILD.bazel", "BUILD"):
        if dir.get_child(name).exists:
            return dir.get_child(name)
    return None

def _write_bzl_srcs(rctx):
    root = rctx.path(".")
    prefix = str(root) + "/"
    with_bzl = {}
    pending = [(root, "")]

    for _ in range(1000000):
        if not pending:
            break
        dir, package = pending.pop()
        if _build_file(dir):
            package = str(dir).removeprefix(prefix) if dir != root else ""
        for child in dir.readdir():
            if child.is_dir:
                pending.append((child, package))
            elif child.basename.endswith((".bzl", ".scl")):
                with_bzl[package] = True
    if not with_bzl:
        fail("{} has no .bzl files for bzl_srcs".format(rctx.name))
    for package in with_bzl:
        if not package:
            continue
        build = _build_file(rctx.path(package))
        rctx.file(build, rctx.read(build) + _BZL_SRCS.format(packages = "[]"), executable = False)
    others = sorted(["//{}:dx_bzl_srcs".format(package) for package in with_bzl if package])
    build = _build_file(root) or root.get_child("BUILD.bazel")
    existing = rctx.read(build) if build.exists else ""
    rctx.file(build, existing + _BZL_SRCS.format(packages = repr(others)), executable = False)

def _hub_repo_impl(rctx):
    _write_files(rctx)
    return rctx.repo_metadata(reproducible = True)

hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    doc = "Writes rendered files into a repository that downloads nothing.",
    attrs = {
        "files": attr.string_dict(doc = "Repository path to file content."),
    },
)

def crlf_text(text):
    """Returns text with every line ending as CRLF."""
    return text.replace("\r\n", "\n").replace("\n", "\r\n")

def digest_error(name, sha256, integrity):
    """Returns the error for an archive pinned by neither sha256 nor integrity, or ""."""
    if sha256 or integrity:
        return ""
    return name + ": set sha256 or integrity; an archive without a digest is not pinned"

def _pinned_archive_impl(rctx):
    error = digest_error(rctx.name, rctx.attr.sha256, rctx.attr.integrity)
    if error:
        fail(error)
    if rctx.attr.download_as:
        rctx.download(url = rctx.attr.urls, output = rctx.attr.download_as, sha256 = rctx.attr.sha256, integrity = rctx.attr.integrity, executable = rctx.attr.executable)
    else:
        rctx.download_and_extract(
            url = rctx.attr.urls,
            sha256 = rctx.attr.sha256,
            integrity = rctx.attr.integrity,
            stripPrefix = rctx.attr.strip_prefix,
            type = rctx.attr.type,
        )
    for inner in rctx.attr.inner:
        rctx.extract(inner)
        rctx.delete(inner)
    missing = [path for path in rctx.attr.required + rctx.attr.dropped if not rctx.path(path).exists]
    if missing:
        fail("{} archive does not ship {}".format(rctx.name, ", ".join(missing)))
    for path in rctx.attr.dropped:
        rctx.delete(path)
    _write_files(rctx)
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    for path in rctx.attr.crlf:
        if not rctx.path(path).exists:
            fail("{} archive does not ship {} to convert to CRLF".format(rctx.name, path))
        rctx.file(path, crlf_text(rctx.read(path)), executable = False)
    if rctx.attr.bzl_srcs:
        _write_bzl_srcs(rctx)
    return rctx.repo_metadata(reproducible = True)

def _under(output, path):
    return path if output == "." else output + "/" + path

def gem_extract(rctx, url, sha256, canonical_id, output, required):
    """Unpacks the data.tar.gz of a pinned .gem into output and checks the required paths."""
    rctx.download_and_extract(
        url = url,
        output = "_gem",
        sha256 = sha256,
        type = "tar",
        canonical_id = canonical_id,
    )
    data = "_gem/data.tar.gz"
    if not rctx.path(data).exists:
        fail("{}: '{}' holds no data.tar.gz".format(rctx.name, url))
    rctx.extract(archive = data, output = output)
    rctx.delete("_gem")
    for path in required:
        if not rctx.path(_under(output, path)).exists:
            fail("{}: '{}' holds no {}".format(rctx.name, url, path))

pinned_archive = repository_rule(
    implementation = _pinned_archive_impl,
    doc = "Fetches one pinned archive, prunes it, writes rendered files and applies patches.",
    attrs = {
        "bzl_srcs": attr.bool(doc = "Adds a public dx_bzl_srcs filegroup of the Starlark files to each package; the root one holds them all."),
        "crlf": attr.string_list(doc = "Paths rewritten with CRLF line endings after patching."),
        "download_as": attr.string(doc = "File name to download to without extracting."),
        "dropped": attr.string_list(doc = "Paths deleted after extraction."),
        "executable": attr.bool(doc = "Marks the file named by download_as executable."),
        "files": attr.string_dict(doc = "Repository path to file content."),
        "integrity": attr.string(doc = "Subresource integrity of the archive, such as sha512-<base64>, used when there is no sha256."),
        "inner": attr.string_list(doc = "Archives inside the downloaded one, extracted into the repository and then deleted."),
        "patches": attr.label_list(doc = "Patches applied last with strip 1."),
        "required": attr.string_list(doc = "Paths the archive must ship."),
        "sha256": attr.string(doc = "Archive digest; set this or integrity."),
        "strip_prefix": attr.string(doc = "Archive directory prefix to strip."),
        "type": attr.string(doc = "Archive type when the URL does not show it."),
        "urls": attr.string_list(mandatory = True, doc = "Archive URLs, tried in order."),
    },
)
