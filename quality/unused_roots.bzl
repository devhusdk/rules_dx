"""Roots of the unused-code closure: every source reachable from a target, its mains, its public sources and its dx-keep sources."""

DxUnusedRootsInfo = provider(
    doc = "Files a cross-file unused-code tool needs besides the target's own sources.",
    fields = {
        "sources": "depset[File]: workspace sources of the target and everything it depends on.",
        "mains": "depset[File]: entry points of the *_binary and *_test rules in the closure.",
        "public": "depset[File]: sources of targets visible to //visibility:public.",
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
    for name in LINKS:
        found = getattr(attrs, name, [])
        links += found if type(found) == "list" else [found]
    below = [t[DxUnusedRootsInfo] for t in links if DxUnusedRootsInfo in t]
    tags = getattr(attrs, "tags", [])
    return [DxUnusedRootsInfo(
        sources = depset(srcs, transitive = [i.sources for i in below]),
        mains = depset(unused_main_files(ctx.rule.kind, main, srcs), transitive = [i.mains for i in below]),
        public = depset(srcs if unused_is_public(getattr(attrs, "visibility", None)) else [], transitive = [i.public for i in below]),
        keep = depset(srcs if KEEP_TAG in tags else [], transitive = [i.keep for i in below]),
    )]

unused_roots_aspect = aspect(
    implementation = _unused_roots_impl,
    doc = "Collects the closure sources and the unused-code roots along deps, the dx forwarders' upstream and the rules_py venv.",
    attr_aspects = LINKS,
    provides = [DxUnusedRootsInfo],
)
