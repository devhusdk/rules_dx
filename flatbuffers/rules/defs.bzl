"""FlatBuffers schema ownership for dx quality."""

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
