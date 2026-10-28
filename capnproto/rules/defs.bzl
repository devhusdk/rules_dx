"""Cap'n Proto schema ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

CapnpInfo = provider(
    doc = "Cap'n Proto schemas of one library and its transitive dependencies.",
    fields = {
        "direct_srcs": "List[File]: schemas owned by this library.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "transitive_imports": "depset[struct(path, file)]: every schema an import can reach, with its path from the import root.",
    },
)

_STD_MARKER = "/src/capnp/"

def std_import_path(short_path):
    """Maps a bundled schema's short_path to its import path under /capnp/.

    Args:
      short_path: short_path of a file from the capnp archive (`../<repo>/src/capnp/...`).

    Returns:
      The path from the import root, for example `capnp/c++.capnp`.
    """
    at = short_path.rfind(_STD_MARKER)
    if at < 0:
        fail("not a bundled capnp schema: " + short_path)
    return "capnp/" + short_path[at + len(_STD_MARKER):]

def _capnp_library_impl(ctx):
    direct = ctx.files.srcs
    for f in direct:
        if f.short_path.startswith("../"):
            fail("capnp_library %s: schema %s is in another Bazel repository; cross-repo capnp schemas are unsupported because imports resolve from the main workspace root" % (ctx.label, f.short_path))
    own = [struct(path = f.short_path, file = f) for f in direct]
    std = [struct(path = std_import_path(f.short_path), file = f) for f in ctx.files._std]
    transitive = depset(
        direct,
        transitive = [dep[CapnpInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    imports = depset(
        own + std,
        transitive = [dep[CapnpInfo].transitive_imports for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(direct)),
        CapnpInfo(direct_srcs = direct, transitive_srcs = transitive, transitive_imports = imports),
        dx_quality_sources(direct, [("capnp", ["capnp"])], str(ctx.label)),
    ]

capnp_library = rule(
    implementation = _capnp_library_impl,
    doc = "Owns Cap'n Proto schemas for dx lint. Imports resolve only from deps, relative to the importing file or from the workspace root, plus the bundled /capnp/*.capnp schemas.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".capnp"], doc = "Cap'n Proto schema files of this package."),
        "deps": attr.label_list(providers = [CapnpInfo], doc = "capnp_library targets whose schemas these import."),
        "_std": attr.label(default = "//capnproto:std", allow_files = True),
    },
    provides = [CapnpInfo],
)
