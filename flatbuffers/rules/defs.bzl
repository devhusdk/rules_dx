"""FlatBuffers schema ownership for dx quality."""

load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

FlatBuffersInfo = provider(
    doc = "FlatBuffers schemas of one library and its transitive dependencies.",
    fields = {
        "direct_srcs": "List[File]: schemas owned by this library.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
    },
)

def _flatbuffers_library_impl(ctx):
    transitive = depset(
        ctx.files.srcs,
        transitive = [dep[FlatBuffersInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        FlatBuffersInfo(direct_srcs = ctx.files.srcs, transitive_srcs = transitive),
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
    """Picks the root schema of a flatbuffers_library.

    Args:
      rule_kind: Rule name for error text.
      label: Label of the calling target.
      library_target: The flatbuffers_library Target.
      root_file: Explicit root File or None.

    Returns:
      The root schema File: root_file, or the only schema the library owns.
    """
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
