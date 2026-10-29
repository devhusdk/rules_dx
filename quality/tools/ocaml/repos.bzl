"""The source repository dune builds OCaml tools from."""

load(":pins.bzl", "DUNIVERSE")

# Directories that only hold tests, docs or benchmarks; dropping them keeps the
# source filegroup small and free of files Bazel cannot glob.
_PRUNED = ["test", "tests", "doc", "docs", "bench", "example", "examples", "review_tests"]

_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "srcs",
    srcs = glob(["**"], exclude = ["**/BUILD", "**/BUILD.bazel", "**/WORKSPACE", "**/MODULE.bazel", "**/REPO.bazel"]),
)

filegroup(
    name = "licenses",
    srcs = glob(["*/LICENSE*"]),
)
"""

def patch_package(name):
    """Returns the package directory a patch file name targets: the text before its first dot."""
    return name.split(".")[0]

def _duniverse_impl(rctx):
    packages = json.decode(rctx.attr.packages)
    for package in packages:
        rctx.download_and_extract(
            url = package["url"],
            sha256 = package["sha256"],
            output = "_unpack/" + package["dir"],
        )
        children = rctx.path("_unpack/" + package["dir"]).readdir()
        if len(children) != 1:
            fail("archive of " + package["name"] + " does not have one top-level directory")
        rctx.rename(children[0], package["dir"])
        for pruned in _PRUNED:
            rctx.delete(package["dir"] + "/" + pruned)
    rctx.delete("_unpack")
    for patch in sorted(rctx.attr.patches, key = lambda label: label.name.split("/")[-1]):
        directory = patch_package(patch.name.split("/")[-1])
        if directory not in [p["dir"] for p in packages]:
            fail("patch " + patch.name + " names no package of the duniverse")
        rctx.patch(patch, strip = 1)
    rctx.file("dune-project", "(lang dune 3.11)\n")
    rctx.file("dune", "(vendored_dirs *)\n")
    rctx.file("BUILD.bazel", _BUILD)
    return rctx.repo_metadata(reproducible = True)

dx_ocaml_duniverse = repository_rule(
    implementation = _duniverse_impl,
    doc = "Unpacks the pinned OCaml source archives side by side, applies the patches and exposes them to one dune workspace.",
    attrs = {
        "packages": attr.string(mandatory = True, doc = "JSON list of {dir, name, url, sha256}."),
        "patches": attr.label_list(allow_files = [".patch"], doc = "Patches named <dir>.<description>.patch, applied inside <dir> in file name order."),
    },
)

def duniverse_packages():
    """Returns the DUNIVERSE entries as the JSON the repository rule takes."""
    return json.encode([
        {"dir": p.dir, "name": p.name, "sha256": p.sha256, "url": p.url}
        for p in DUNIVERSE
    ])
