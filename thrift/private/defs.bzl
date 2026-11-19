"""Apache Thrift schema ownership for dx quality."""

load("//core:stage.bzl", "stage_own")
load("//core:wrapper.bzl", "dx_quality_sources")

ThriftInfo = provider(
    doc = "Thrift schemas of one library and its transitive dependencies.",
    fields = {
        "label": "Label: the thrift_library target.",
        "direct_deps": "List[Label]: thrift_library dependencies.",
        "direct_srcs": "List[File]: schemas owned by this library.",
        "direct_staged": "List[File]: the staged copy of each of direct_srcs, in the same order.",
        "stage_root": "string: exec path of the include root direct_staged lives under.",
        "stage_roots": "depset[string]: include roots of this library and every dependency.",
        "staged": "depset[File]: staged schemas under stage_roots.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "transitive_includes": "depset[struct(path, file, staged)]: every schema an include can reach, with its path from the include root and its staged copy.",
    },
)

def _thrift_library_impl(ctx):
    direct = ctx.files.srcs
    for f in direct:
        if f.short_path.startswith("../"):
            fail("thrift_library %s: schema %s is in another Bazel repository; cross-repo thrift schemas are unsupported because includes resolve from the main workspace root" % (ctx.label, f.short_path))
    stage = stage_own(ctx, "_thrift_stage", [struct(path = f.short_path, file = f) for f in direct])
    own = [struct(path = f.short_path, file = f, staged = link) for f, link in zip(direct, stage.staged)]
    deps = [dep[ThriftInfo] for dep in ctx.attr.deps]
    return [
        DefaultInfo(files = depset(direct)),
        ThriftInfo(
            label = ctx.label,
            direct_deps = [dep.label for dep in ctx.attr.deps],
            direct_srcs = direct,
            direct_staged = stage.staged,
            stage_root = stage.root,
            stage_roots = depset([stage.root], transitive = [d.stage_roots for d in deps]),
            staged = depset(stage.staged, transitive = [d.staged for d in deps]),
            transitive_srcs = depset(direct, transitive = [d.transitive_srcs for d in deps]),
            transitive_includes = depset(own, transitive = [d.transitive_includes for d in deps]),
        ),
        dx_quality_sources(direct, [("thrift", ["thrift"])], str(ctx.label)),
    ]

thrift_library = rule(
    implementation = _thrift_library_impl,
    doc = "Owns Apache Thrift schemas for dx lint. Includes resolve only from deps, relative to the including file within its package or from the workspace root.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".thrift"], doc = "Thrift schema files of this package."),
        "deps": attr.label_list(providers = [ThriftInfo], doc = "thrift_library targets whose schemas these include."),
    },
    provides = [ThriftInfo],
)
