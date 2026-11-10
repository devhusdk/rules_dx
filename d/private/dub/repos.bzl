"""Repository rule and hub for DUB packages fetched from dub.selections.json."""

load("//core:repo.bzl", "hub_repo")
load(":lock.bzl", "analyze", "dependency_names", "hub_package_build", "hub_root_build", "pin_entry_json", "spoke_build")

_DEFS_BZL = str(Label("//d/private:defs.bzl"))

def _package_impl(rctx):
    attrs = rctx.attr
    kwargs = {"url": [attrs.url], "output": "_archive.zip"}
    if attrs.sha256:
        kwargs["sha256"] = attrs.sha256
    download = rctx.download(**kwargs)
    if attrs.unpinned:
        rctx.delete("_archive.zip")
        fail("%s %s\nDownloaded sha256 is %s. Review it and commit to dub.digests.json:\n%s" % (
            attrs.package,
            attrs.unpinned,
            download.sha256,
            pin_entry_json(attrs.package, attrs.version, download.sha256),
        ))
    rctx.extract("_archive.zip", output = "_probe")
    top = rctx.path("_probe").readdir()
    strip = ""
    if len(top) == 1 and top[0].is_dir:
        strip = top[0].basename
    rctx.delete("_probe")
    rctx.extract("_archive.zip", strip_prefix = strip)
    rctx.delete("_archive.zip")
    if rctx.path("dub.json").exists:
        manifest = json.decode(rctx.read("dub.json"))
    elif rctx.path("package.json").exists:
        manifest = json.decode(rctx.read("package.json"))
    elif rctx.path("dub.sdl").exists:
        fail("%s %s uses dub.sdl, which is not supported yet; only dub.json packages can be fetched" % (attrs.package, attrs.version))
    else:
        fail("%s %s has no dub.json" % (attrs.package, attrs.version))
    existing = [entry.basename for entry in rctx.path(".").readdir() if entry.is_dir]
    deps = [attrs.selected[name] for name in dependency_names(manifest, attrs.selected)]
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", spoke_build(attrs.package, manifest, existing, deps, _DEFS_BZL))
    return rctx.repo_metadata(reproducible = True)

dub_package = repository_rule(
    implementation = _package_impl,
    doc = "Downloads one selected DUB package and declares it as a d_library.",
    attrs = {
        "package": attr.string(mandatory = True, doc = "The DUB package name."),
        "selected": attr.string_dict(doc = "Repository name of every selected package, by package name."),
        "sha256": attr.string(doc = "The archive digest; empty means unpinned and the fetch fails after printing the digest."),
        "unpinned": attr.string(doc = "Why the package has no usable digest; empty when pinned."),
        "url": attr.string(mandatory = True, doc = "The archive URL."),
        "version": attr.string(mandatory = True, doc = "The selected version."),
    },
)

def declare(hub, selections_text, digests_text):
    """Declares the spokes and the hub of one selections file; returns the analysis."""
    result = analyze(hub, selections_text, digests_text)
    if result.errors:
        fail("dub.selections.json for %s is unusable:\n  %s" % (hub, "\n  ".join(result.errors)))
    selected = {name: package.spoke for name, package in result.packages.items()}
    for name in sorted(result.packages.keys()):
        package = result.packages[name]
        dub_package(
            name = package.spoke,
            package = name,
            selected = selected,
            sha256 = package.sha256,
            unpinned = package.unpinned,
            url = package.url,
            version = package.version,
        )
    files = {"BUILD.bazel": hub_root_build(result.packages)}
    for name, package in result.packages.items():
        files[name + "/BUILD.bazel"] = hub_package_build(name, package.spoke)
    hub_repo(name = hub, files = files)
    return result
