"""Cap'n Proto schema ownership for dx quality."""

load("//core:stage.bzl", "stage_own")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "CapnpStdInfo", "CapnprotoInfo")

_STD_MARKER = "/src/capnp/"

# capnpc-java ships java.capnp under compiler/src/main/schema/capnp/.
_STD_JAVA_MARKER = "/schema/capnp/"

def std_import_path(short_path):
    """Maps a bundled schema's short_path to its import path under /capnp/."""
    for marker in (_STD_MARKER, _STD_JAVA_MARKER):
        at = short_path.rfind(marker)
        if at >= 0:
            return "capnp/" + short_path[at + len(marker):]
    fail("not a bundled capnp schema: " + short_path)

def _std_stage_impl(ctx):
    entries = [struct(path = std_import_path(f.short_path), file = f) for f in ctx.files.srcs]
    stage = stage_own(ctx, "_capnp_std", entries)
    return [CapnpStdInfo(
        imports = [struct(path = e.path, file = e.file, staged = link) for e, link in zip(entries, stage.staged)],
        stage_root = stage.root,
    )]

capnproto_std_stage = rule(
    implementation = _std_stage_impl,
    doc = "Stages the bundled Cap'n Proto schemas under their /capnp/ import paths.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".capnp"], doc = "The bundled schemas."),
    },
    provides = [CapnpStdInfo],
)

def _capnp_library_impl(ctx):
    direct = ctx.files.srcs
    for f in direct:
        if f.short_path.startswith("../"):
            fail("capnproto_library %s: schema %s is in another Bazel repository; cross-repo capnp schemas are unsupported because imports resolve from the main workspace root" % (ctx.label, f.short_path))
    stage = stage_own(ctx, "_capnp_stage", [struct(path = f.short_path, file = f) for f in direct])
    own = [struct(path = f.short_path, file = f, staged = link) for f, link in zip(direct, stage.staged)]
    std = ctx.attr._std[CapnpStdInfo]
    deps = [dep[CapnprotoInfo] for dep in ctx.attr.deps]
    return [
        DefaultInfo(files = depset(direct)),
        CapnprotoInfo(
            label = str(ctx.label),
            direct_deps = deps,
            direct_srcs = direct,
            direct_staged = stage.staged,
            stage_root = stage.root,
            stage_roots = depset([stage.root, std.stage_root], transitive = [d.stage_roots for d in deps]),
            staged = depset(stage.staged + [e.staged for e in std.imports], transitive = [d.staged for d in deps]),
            transitive_srcs = depset(direct, transitive = [d.transitive_srcs for d in deps]),
            transitive_imports = depset(own + std.imports, transitive = [d.transitive_imports for d in deps]),
        ),
        dx_quality_sources(direct, [("capnp", ["capnp"])], str(ctx.label)),
    ]

capnproto_library = rule(
    implementation = _capnp_library_impl,
    doc = "Owns Cap'n Proto schemas for dx lint. Imports resolve only from deps, relative to the importing file within its package or from the workspace root, plus the bundled /capnp/*.capnp schemas.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".capnp"], doc = "Cap'n Proto schema files of this package."),
        "deps": attr.label_list(providers = [CapnprotoInfo], doc = "capnproto_library targets whose schemas these import."),
        "_std": attr.label(default = Label("//capnproto/private:std_stage"), providers = [CapnpStdInfo]),
    },
    provides = [CapnprotoInfo],
)
