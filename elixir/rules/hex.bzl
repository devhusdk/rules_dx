"""Pinned Hex package repositories."""

load(":mix_lock.bzl", "parse_terms")

HEX_REPO = "https://repo.hex.pm/tarballs/"

def hex_tarball_url(package, version):
    """Returns the repo.hex.pm tarball URL for one package release."""
    return HEX_REPO + package + "-" + version + ".tar"

def hex_metadata_entry(key, value):
    """Returns the metadata.config term one package release must carry."""
    return '{<<"' + key + '">>,<<"' + value + '">>}.'

def erlang_terms(text):
    """Returns the nested lists of an Erlang term file: binaries become strings, atoms stay words."""
    lines = [line for line in text.split("\n") if not line.strip().startswith("%")]
    text = "\n".join(lines).replace('<<"', '"').replace('">>', '"')
    return parse_terms(text.replace("}.", "}").replace("].", "]"))

def hex_requirements(package, metadata):
    """Returns the name and optional flag of each requirement of a release's metadata.config."""
    found = []
    for term in erlang_terms(metadata):
        if term[0] != "requirements":
            continue
        for requirement in term[1]:
            fields = {pair[0]: pair[1] for pair in requirement}
            if "name" not in fields:
                fail("hex package {} has a requirement without a name".format(package))
            found.append((fields["name"], fields.get("optional") == "true"))
    return found

def hex_dependency_labels(package, metadata, repos):
    """Returns the labels of the requirements of a release, from the map of locked package to label.

    A requirement missing from the map fails unless it is optional."""
    labels = []
    for name, optional in hex_requirements(package, metadata):
        if name in repos:
            labels.append(repos[name])
        elif not optional:
            fail("hex package {} requires {}, which is not in the lock".format(package, name))
    return sorted(labels)

def hex_extract(rctx, package, version, sha256, output = "."):
    """Downloads one pinned Hex release, checks its metadata, extracts the sources and returns the metadata."""
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
    return metadata

def _hex_package_impl(rctx):
    modes = [rctx.attr.build_file != None, rctx.attr.build_file_content != "", rctx.attr.build_file_template != ""]
    if modes.count(True) != 1:
        fail("hex_package needs exactly one of build_file, build_file_content and build_file_template")
    metadata = hex_extract(rctx, rctx.attr.package, rctx.attr.version, rctx.attr.sha256)
    rctx.file("REPO.bazel", "")
    if rctx.attr.build_file != None:
        rctx.file("BUILD.bazel", rctx.read(rctx.attr.build_file))
    elif rctx.attr.build_file_template != "":
        deps = hex_dependency_labels(rctx.attr.package, metadata, rctx.attr.dependency_labels)
        rctx.file("BUILD.bazel", rctx.attr.build_file_template.replace("{deps}", repr(deps)))
    else:
        rctx.file("BUILD.bazel", rctx.attr.build_file_content)
    return rctx.repo_metadata(reproducible = True)

hex_package = repository_rule(
    implementation = _hex_package_impl,
    doc = "Fetches one pinned package release from repo.hex.pm.",
    attrs = {
        "build_file": attr.label(allow_single_file = True, doc = "BUILD file for the package sources."),
        "build_file_template": attr.string(doc = "BUILD file text whose {deps} becomes the labels of the release's metadata.config requirements."),
        "build_file_content": attr.string(doc = "BUILD file text for the package sources."),
        "dependency_labels": attr.string_dict(doc = "Label of each locked package by name, for build_file_template."),
        "package": attr.string(mandatory = True, doc = "Hex package name."),
        "sha256": attr.string(mandatory = True, doc = "Digest of the release tarball."),
        "version": attr.string(mandatory = True, doc = "Exact release version."),
    },
)
