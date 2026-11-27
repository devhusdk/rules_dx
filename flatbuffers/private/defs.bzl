"""FlatBuffers schema ownership for dx quality."""

load("//core:stage.bzl", "stage_own")
load("//core:wrapper.bzl", "dx_quality_sources")
load("//core/private/testing:providers.bzl", "display_label")

FlatBuffersInfo = provider(
    doc = "FlatBuffers schemas of one library and its transitive dependencies.",
    fields = {
        "direct_srcs": "List[File]: schemas owned by this library.",
        "direct_staged": "List[File]: the staged copy of each of direct_srcs, in the same order.",
        "stage_root": "string: exec path of the include root direct_staged lives under.",
        "stage_roots": "depset[string]: include roots of this library and every dependency.",
        "staged": "depset[File]: staged schemas under stage_roots.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
    },
)

def _flatbuffers_library_impl(ctx):
    for f in ctx.files.srcs:
        if f.short_path.startswith("../"):
            fail("flatbuffers_library %s: schema %s is in another Bazel repository; cross-repo flatbuffers schemas are unsupported because includes resolve from the main workspace root" % (ctx.label, f.short_path))
    stage = stage_own(ctx, "_fbs_stage", [struct(path = f.short_path, file = f) for f in ctx.files.srcs])
    deps = [dep[FlatBuffersInfo] for dep in ctx.attr.deps]
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        FlatBuffersInfo(
            direct_srcs = ctx.files.srcs,
            direct_staged = stage.staged,
            stage_root = stage.root,
            stage_roots = depset([stage.root], transitive = [d.stage_roots for d in deps]),
            staged = depset(stage.staged, transitive = [d.staged for d in deps]),
            transitive_srcs = depset(ctx.files.srcs, transitive = [d.transitive_srcs for d in deps]),
        ),
        dx_quality_sources(ctx.files.srcs, [("flatbuffers", ["fbs"])], str(ctx.label)),
    ]

flatbuffers_library = rule(
    implementation = _flatbuffers_library_impl,
    doc = "Owns FlatBuffers schemas for dx lint. Includes resolve only from deps or the including file's directory.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".fbs"], doc = "FlatBuffers schema files of this package."),
        "deps": attr.label_list(providers = [FlatBuffersInfo], doc = "flatbuffers_library targets whose schemas these include."),
    },
    provides = [FlatBuffersInfo],
)

def resolve_root(rule_kind, label, library_target, root_file):
    """Picks the root schema of a flatbuffers_library."""
    owned = library_target[FlatBuffersInfo].direct_srcs
    if root_file == None:
        if len(owned) != 1:
            fail(
                rule_kind + " " + display_label(label) + ": root is required because " +
                display_label(library_target.label) + " owns " + str(len(owned)) + " schemas (want exactly one root)",
            )
        return owned[0]
    if root_file not in owned:
        fail(
            rule_kind + " " + display_label(label) + ": root " + root_file.short_path +
            " is not owned by " + display_label(library_target.label),
        )
    return root_file
