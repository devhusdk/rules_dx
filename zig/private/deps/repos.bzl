"""Repository rule and hub for Zig packages fetched from build.zig.zon."""

load("//core:pins.bzl", "fetch_and_extract", "pin_entry_json")
load("//core:repo.bzl", "hub_repo")
load(":lock.bzl", "analyze", "archive_type", "dependency_names", "hub_package_build", "hub_root_build", "main_candidates", "main_file", "spoke_build")

_DEFS_BZL = str(Label("//zig/private:defs.bzl"))

def _package_impl(rctx):
    attrs = rctx.attr
    archive = "_archive." + archive_type(attrs.url)
    fetch_and_extract(rctx, archive, lambda sha256: pin_entry_json(attrs.package, "hash", attrs.hash, sha256))
    existing = []
    for suffix in ["", "src/"]:
        directory = rctx.path(suffix) if suffix else rctx.path(".")
        if directory.exists:
            existing += [suffix + entry.basename for entry in directory.readdir() if not entry.is_dir]
    main = main_file(attrs.package, existing)
    if not main:
        fail("%s has no root source file; expected one of %s" % (attrs.package, ", ".join(main_candidates(attrs.package))))
    zon = rctx.read("build.zig.zon") if rctx.path("build.zig.zon").exists else ""
    names, problems = dependency_names(zon, attrs.selected_hashes)
    if problems:
        fail("%s %s" % (attrs.package, ("\n" + attrs.package + " ").join(problems)))
    deps = [attrs.selected[name] for name in names]
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", spoke_build(attrs.package, main, deps, _DEFS_BZL))
    return rctx.repo_metadata(reproducible = True)

zig_package = repository_rule(
    implementation = _package_impl,
    doc = "Downloads one build.zig.zon dependency and declares it as a zig_library.",
    attrs = {
        "hash": attr.string(mandatory = True, doc = "The .hash of the dependency in build.zig.zon."),
        "package": attr.string(mandatory = True, doc = "The dependency name."),
        "selected": attr.string_dict(doc = "Repository name of every dependency, by name."),
        "selected_hashes": attr.string_dict(doc = "The .hash of every dependency, by name."),
        "sha256": attr.string(doc = "The archive digest; empty means unpinned and the fetch fails after printing the digest."),
        "unpinned": attr.string(doc = "Why the package has no usable digest; empty when pinned."),
        "url": attr.string(mandatory = True, doc = "The archive URL."),
    },
)

def declare(hub, zon_text, digests_text):
    """Declares the spokes and the hub of one manifest; returns the analysis."""
    result = analyze(hub, zon_text, digests_text)
    if result.errors:
        fail("build.zig.zon for %s is unusable:\n  %s" % (hub, "\n  ".join(result.errors)))
    selected = {name: package.spoke for name, package in result.packages.items()}
    hashes = {name: package.hash for name, package in result.packages.items()}
    for name in sorted(result.packages.keys()):
        package = result.packages[name]
        zig_package(
            name = package.spoke,
            hash = package.hash,
            package = name,
            selected = selected,
            selected_hashes = hashes,
            sha256 = package.sha256,
            unpinned = package.unpinned,
            url = package.url,
        )
    files = {"BUILD.bazel": hub_root_build(result.packages)}
    for name, package in result.packages.items():
        files[name + "/BUILD.bazel"] = hub_package_build(name, package.spoke)
    hub_repo(name = hub, files = files)
    return result
