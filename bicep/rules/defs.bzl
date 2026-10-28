"""Bicep source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "BicepInfo")

def _bicep_module_impl(ctx):
    for f in ctx.files.data:
        if f.basename == "bicepconfig.json":
            fail("bicep_module: {} cannot be listed in data; bind it with bicep_config".format(f.short_path))
    transitive_srcs = depset(
        ctx.files.srcs,
        transitive = [dep[BicepInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    transitive_data = depset(
        ctx.files.data,
        transitive = [dep[BicepInfo].transitive_data for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(transitive = [transitive_srcs, transitive_data])),
        BicepInfo(transitive_srcs = transitive_srcs, transitive_data = transitive_data),
        dx_quality_sources(ctx.files.srcs, [("bicep", ["bicep", "bicepparam"])], str(ctx.label)),
    ]

bicep_module = rule(
    implementation = _bicep_module_impl,
    doc = "Owns the Bicep files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".bicep", ".bicepparam"], doc = "Bicep and Bicep parameter files of this package."),
        "data": attr.label_list(allow_files = True, doc = "Files read by load functions; not checked by dx quality."),
        "deps": attr.label_list(providers = [BicepInfo], doc = "Bicep modules this package depends on."),
    },
)
