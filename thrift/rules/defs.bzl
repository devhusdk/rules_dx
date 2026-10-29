"""Apache Thrift schema ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

ThriftInfo = provider(
    doc = "Thrift schemas of one library and its transitive dependencies.",
    fields = {
        "label": "Label: the thrift_library target.",
        "direct_deps": "List[Label]: thrift_library dependencies.",
        "direct_srcs": "List[File]: schemas owned by this library.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "transitive_includes": "depset[struct(path, file)]: every schema an include can reach, with its path from the include root.",
    },
)

def _thrift_library_impl(ctx):
    direct = ctx.files.srcs
    for f in direct:
        if f.short_path.startswith("../"):
            fail("thrift_library %s: schema %s is in another Bazel repository; cross-repo thrift schemas are unsupported because includes resolve from the main workspace root" % (ctx.label, f.short_path))
    own = [struct(path = f.short_path, file = f) for f in direct]
    transitive = depset(
        direct,
        transitive = [dep[ThriftInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    includes = depset(
        own,
        transitive = [dep[ThriftInfo].transitive_includes for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(direct)),
        ThriftInfo(
            label = ctx.label,
            direct_deps = [dep.label for dep in ctx.attr.deps],
            direct_srcs = direct,
            transitive_srcs = transitive,
            transitive_includes = includes,
        ),
        dx_quality_sources(direct, [("thrift", ["thrift"])], str(ctx.label)),
    ]

thrift_library = rule(
    implementation = _thrift_library_impl,
    doc = "Owns Apache Thrift schemas for dx lint. Includes resolve only from deps, relative to the including file or from the workspace root.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".thrift"], doc = "Thrift schema files of this package."),
        "deps": attr.label_list(providers = [ThriftInfo], doc = "thrift_library targets whose schemas these include."),
    },
    provides = [ThriftInfo],
)
