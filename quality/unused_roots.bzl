"""Roots of the unused-code closure: every source reachable from a target, its mains, its public sources and its dx-keep sources."""

load("//quality:real_core.bzl", "real_context")

DxUnusedRootsInfo = provider(
    doc = "Files a cross-file unused-code tool needs besides the target's own sources.",
    fields = {
        "sources": "depset[File]: workspace sources of the target and everything it depends on.",
        "mains": "depset[File]: entry points of the *_binary and *_test rules in the closure.",
        "public": "depset[File]: sources of targets visible to //visibility:public.",
        "public_below": "depset[File]: the public sources of the dependencies alone, for a forwarder that must not inherit its upstream's visibility.",
        "keep": "depset[File]: sources of targets tagged dx-keep.",
    },
)

LINKS = ["deps", "upstream", "venv"]
KEEP_TAG = "dx-keep"
PUBLIC_VISIBILITY = "//visibility:public"

def unused_is_public(visibility):
    """Tells whether a visibility attribute value contains //visibility:public."""
    if visibility == None:
        return False
    return PUBLIC_VISIBILITY in [str(entry).lstrip("@") for entry in visibility]

def unused_main_files(kind, main, srcs):
    """Returns the entry-point files of one rule: its main, or its srcs for binaries and tests without one."""
    if len(main) > 0:
        return list(main)
    if kind.endswith("_binary") or kind.endswith("_test"):
        return list(srcs)
    return []

def _workspace(files):
    return [f for f in files if f.is_source and ".." not in f.short_path.split("/")]

def _files(ctx, name):
    return getattr(ctx.rule.files, name, [])

def _unused_roots_impl(target, ctx):
    attrs = ctx.rule.attr
    srcs = _workspace(_files(ctx, "srcs"))
    main = _workspace(_files(ctx, "main"))
    links = []
    forwarded = []
    for name in LINKS:
        found = getattr(attrs, name, [])
        found = found if type(found) == "list" else [found]
        links += found
        if name == "upstream":
            forwarded += found
    below = [t[DxUnusedRootsInfo] for t in links if DxUnusedRootsInfo in t]
    tags = getattr(attrs, "tags", [])
    below_public = [
        t[DxUnusedRootsInfo].public_below if t in forwarded else t[DxUnusedRootsInfo].public
        for t in links
        if DxUnusedRootsInfo in t
    ]
    return [DxUnusedRootsInfo(
        sources = depset(srcs, transitive = [i.sources for i in below]),
        mains = depset(unused_main_files(ctx.rule.kind, main, srcs), transitive = [i.mains for i in below]),
        public = depset(srcs if unused_is_public(getattr(attrs, "visibility", None)) else [], transitive = below_public),
        public_below = depset(transitive = below_public),
        keep = depset(srcs if KEEP_TAG in tags else [], transitive = [i.keep for i in below]),
    )]

unused_roots_aspect = aspect(
    implementation = _unused_roots_impl,
    doc = "Collects the closure sources and the unused-code roots along deps, the dx forwarders' upstream and the rules_py venv.",
    attr_aspects = LINKS,
    provides = [DxUnusedRootsInfo],
)

def unused_roots_paths(files, extensions):
    """Sorted workspace paths of the files with one of the extensions."""
    return sorted({f.short_path: True for f in files.to_list() if f.extension in extensions}.keys())

def unused_roots_context(target, ctx, extensions):
    """Stages dx-unused/roots.json with the mains, public and keep paths of the closure."""
    if DxUnusedRootsInfo not in target:
        return real_context()
    roots = target[DxUnusedRootsInfo]
    out = ctx.actions.declare_file(target.label.name + "-dx-unused-roots.json")
    ctx.actions.write(out, json.encode({
        "keep": unused_roots_paths(roots.keep, extensions),
        "mains": unused_roots_paths(roots.mains, extensions),
        "public": unused_roots_paths(roots.public, extensions),
    }))
    return real_context(tool_files = [("dx-unused/roots.json", out)], inputs = [out])

def unused_closure_sources(target, capability, direct_sources, classes):
    """Replaces the direct sources of an unused run with the closure, grouped by extension into classes."""
    if capability != "unused" or DxUnusedRootsInfo not in target:
        return direct_sources
    grouped = {}
    for f in target[DxUnusedRootsInfo].sources.to_list():
        if f.extension in classes:
            grouped.setdefault(classes[f.extension], []).append(f)
    found = {name: depset(files) for name, files in grouped.items()}
    return found if found else direct_sources
