"""Pinned Hex package repositories."""

HEX_REPO = "https://repo.hex.pm/tarballs/"

def hex_tarball_url(package, version):
    """Returns the repo.hex.pm tarball URL for one package release."""
    return HEX_REPO + package + "-" + version + ".tar"

def hex_metadata_entry(key, value):
    """Returns the metadata.config term one package release must carry."""
    return '{<<"' + key + '">>,<<"' + value + '">>}.'

def hex_extract(rctx, package, version, sha256, output = "."):
    """Downloads one pinned Hex release, checks its metadata and extracts the sources."""
    scratch = "_hex_" + package
    rctx.download_and_extract(
        url = hex_tarball_url(package, version),
        sha256 = sha256,
        type = "tar",
        output = scratch,
    )
    metadata = rctx.read(scratch + "/metadata.config")
    for key, value in [("name", package), ("version", version)]:
        if hex_metadata_entry(key, value) not in metadata:
            fail("hex package {} {} does not declare {} {}".format(package, version, key, value))
    rctx.extract(scratch + "/contents.tar.gz", output = output)
    rctx.delete(scratch)

def _hex_package_impl(rctx):
    if (rctx.attr.build_file == None) == (rctx.attr.build_file_content == ""):
        fail("hex_package needs exactly one of build_file and build_file_content")
    hex_extract(rctx, rctx.attr.package, rctx.attr.version, rctx.attr.sha256)
    rctx.file("REPO.bazel", "")
    if rctx.attr.build_file != None:
        rctx.file("BUILD.bazel", rctx.read(rctx.attr.build_file))
    else:
        rctx.file("BUILD.bazel", rctx.attr.build_file_content)
    return rctx.repo_metadata(reproducible = True)

hex_package = repository_rule(
    implementation = _hex_package_impl,
    doc = "Fetches one pinned package release from repo.hex.pm.",
    attrs = {
        "build_file": attr.label(allow_single_file = True, doc = "BUILD file for the package sources."),
        "build_file_content": attr.string(doc = "BUILD file text for the package sources."),
        "package": attr.string(mandatory = True, doc = "Hex package name."),
        "sha256": attr.string(mandatory = True, doc = "Digest of the release tarball."),
        "version": attr.string(mandatory = True, doc = "Exact release version."),
    },
)
