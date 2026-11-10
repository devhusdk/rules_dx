"""Repository rule and hub for Composer packages fetched from composer.lock."""

load("//core:pins.bzl", "fetch_and_extract", "pin_entry_json")
load("//core:repo.bzl", "hub_repo")
load(":lock.bzl", "analyze", "hub_package_build", "hub_root_build", "spoke_build")

_PACKAGE_BZL = str(Label("//php/private:php.bzl"))

def _package_impl(rctx):
    attrs = rctx.attr
    fetch_and_extract(rctx, "_archive", lambda sha256: pin_entry_json(attrs.package, "reference", attrs.reference, sha256), type = attrs.archive_type)
    rctx.file("REPO.bazel", "")
    rctx.file("manifest.json", attrs.manifest + "\n")
    package = struct(name = attrs.package, version = attrs.version)
    rctx.file("BUILD.bazel", spoke_build(package, _PACKAGE_BZL, attrs.dep_spokes))
    return rctx.repo_metadata(reproducible = True)

composer_package = repository_rule(
    implementation = _package_impl,
    doc = "Downloads one locked Composer package and declares it as a php_composer_package.",
    attrs = {
        "archive_type": attr.string(mandatory = True, doc = "The archive type, zip or tar."),
        "dep_spokes": attr.string_list(doc = "Repository names of the packages this one requires."),
        "manifest": attr.string(mandatory = True, doc = "The filtered composer.lock entry as JSON."),
        "package": attr.string(mandatory = True, doc = "The Composer package name."),
        "reference": attr.string(doc = "The dist reference from composer.lock."),
        "sha256": attr.string(doc = "The archive digest; empty means unpinned and the fetch fails after printing the digest."),
        "unpinned": attr.string(doc = "Why the package has no usable digest; empty when pinned."),
        "urls": attr.string_list(mandatory = True, doc = "Archive URLs, tried in order."),
        "version": attr.string(mandatory = True, doc = "The locked version."),
    },
)

def declare(hub, lock_text, sidecar_text):
    """Declares the spokes and the hub of one lock; returns the analysis."""
    result = analyze(hub, lock_text, sidecar_text)
    if result.errors:
        fail("composer.lock for %s is unusable:\n  %s" % (hub, "\n  ".join(result.errors)))
    for name in sorted(result.packages.keys()):
        package = result.packages[name]
        composer_package(
            name = package.spoke,
            archive_type = package.archive_type,
            dep_spokes = [result.packages[dep].spoke for dep in package.deps],
            manifest = json.encode(package.manifest),
            package = name,
            reference = package.reference,
            sha256 = package.sha256,
            unpinned = package.unpinned,
            urls = package.urls,
            version = package.version,
        )
    info = {name: struct(dev = package.dev, spoke = package.spoke) for name, package in result.packages.items()}
    files = {"BUILD.bazel": hub_root_build(info, _PACKAGE_BZL)}
    for name, package in result.packages.items():
        files[name + "/BUILD.bazel"] = hub_package_build(name, package.spoke)
    hub_repo(name = hub, files = files)
    return result
