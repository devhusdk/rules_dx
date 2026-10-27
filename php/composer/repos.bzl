"""Repository rules and hub for Composer packages fetched from composer.lock."""

load(":lock.bzl", "analyze", "hub_package_build", "hub_root_build", "pin_entry_json", "spoke_build")

_PACKAGE_BZL = str(Label("//php/rules:php.bzl"))

def _remove_nested_build_files(rctx):
    """Deletes BUILD files below the package root so no subdirectory becomes a Bazel package.

    A nested package would drop its files from the spoke's glob, while the autoload action stages
    every file it finds. Removing them keeps declared inputs and staged files the same set.
    """
    pending = [rctx.path(".")]
    for _ in range(1000000):
        if not pending:
            break
        directory = pending.pop()
        for entry in directory.readdir():
            if entry.is_dir:
                pending.append(entry)
            elif entry.basename in ("BUILD", "BUILD.bazel") and directory != rctx.path("."):
                rctx.delete(entry)

def _package_impl(rctx):
    attrs = rctx.attr
    kwargs = {"type": attrs.archive_type, "url": attrs.urls}
    if attrs.sha256:
        kwargs["sha256"] = attrs.sha256
    probe = rctx.download_and_extract(output = "_probe", **kwargs)
    if attrs.unpinned:
        rctx.delete("_probe")
        fail("%s %s\nDownloaded sha256 is %s. Review it and commit to composer.digests.json:\n%s" % (
            attrs.package,
            attrs.unpinned,
            probe.sha256,
            pin_entry_json(attrs.package, attrs.reference, probe.sha256),
        ))
    top = rctx.path("_probe").readdir()
    strip = ""
    if len(top) == 1 and top[0].is_dir:
        strip = top[0].basename
    rctx.delete("_probe")
    if strip:
        kwargs["strip_prefix"] = strip
    rctx.download_and_extract(**kwargs)
    _remove_nested_build_files(rctx)
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

def _hub_impl(rctx):
    packages = json.decode(rctx.attr.packages)
    rctx.file("REPO.bazel", "")
    groups = str(Label("//php/rules:php.bzl"))
    info = {name: struct(dev = dev, spoke = spoke) for name, (dev, spoke) in packages.items()}
    rctx.file("BUILD.bazel", hub_root_build(info, groups))
    for name, (_, spoke) in packages.items():
        rctx.file(name + "/BUILD.bazel", hub_package_build(name, spoke))
    return rctx.repo_metadata(reproducible = True)

composer_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Gives every locked package the label @<hub>//<vendor>/<name>.",
    attrs = {
        "packages": attr.string(mandatory = True, doc = "JSON object of package name to [dev, spoke]."),
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
    composer_hub(
        name = hub,
        packages = json.encode({name: [package.dev, package.spoke] for name, package in result.packages.items()}),
    )
    return result
