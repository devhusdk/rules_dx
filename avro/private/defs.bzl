"""Apache Avro schema ownership for dx quality."""

load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "AvroInfo")

def _avro_library_impl(ctx):
    direct = ctx.files.srcs
    transitive = depset(
        direct,
        order = "topological",
        transitive = [dep[AvroInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(direct)),
        AvroInfo(
            label = ctx.label,
            direct_deps = [dep.label for dep in ctx.attr.deps],
            direct_srcs = direct,
            transitive_srcs = transitive,
            import_root = ".",
        ),
        dx_quality_sources(direct, [("avro", ["avsc", "avdl"])], str(ctx.label)),
    ]

avro_library = rule(
    implementation = _avro_library_impl,
    doc = "Owns Apache Avro schemas for dx lint.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".avsc", ".avdl"], doc = "Avro schema files of this package."),
        "deps": attr.label_list(providers = [AvroInfo], doc = "avro_library targets whose schemas these import."),
    },
    provides = [AvroInfo],
)
