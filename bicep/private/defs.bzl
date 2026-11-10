"""Bicep source ownership for dx quality."""

load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "BicepInfo", "BicepRegistryInfo")

def _bicep_library_impl(ctx):
    for f in ctx.files.data:
        if f.basename == "bicepconfig.json":
            fail("bicep_library {}: {} cannot be listed in data; bind it with bicep_config".format(ctx.label, f.short_path))
    transitive_srcs = depset(
        ctx.files.srcs,
        transitive = [dep[BicepInfo].transitive_srcs for dep in ctx.attr.deps if BicepInfo in dep],
    )
    transitive_data = depset(
        ctx.files.data,
        transitive = [dep[BicepInfo].transitive_data for dep in ctx.attr.deps if BicepInfo in dep],
    )
    transitive_registry = depset(
        transitive = [
            dep[BicepInfo].transitive_registry if BicepInfo in dep else dep[BicepRegistryInfo].files
            for dep in ctx.attr.deps
        ],
    )
    return [
        DefaultInfo(files = depset(transitive = [transitive_srcs, transitive_data, transitive_registry])),
        BicepInfo(transitive_srcs = transitive_srcs, transitive_data = transitive_data, transitive_registry = transitive_registry),
        dx_quality_sources(ctx.files.srcs, [("bicep", ["bicep", "bicepparam"])], str(ctx.label)),
    ]

bicep_library = rule(
    implementation = _bicep_library_impl,
    doc = "Owns the Bicep files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".bicep", ".bicepparam"], doc = "Bicep and Bicep parameter files of this package."),
        "data": attr.label_list(allow_files = True, doc = "Files read by load functions; not checked by dx quality."),
        "deps": attr.label_list(providers = [[BicepInfo], [BicepRegistryInfo]], doc = "Bicep modules and registry modules this package depends on."),
    },
)

def _bicep_registry_module_impl(ctx):
    files = depset(ctx.files.srcs)
    return [
        DefaultInfo(files = files),
        BicepRegistryInfo(ref = ctx.attr.ref, cache_root = ctx.attr.cache_root, files = files),
    ]

bicep_registry_module = rule(
    implementation = _bicep_registry_module_impl,
    doc = "A restored Bicep registry module and its cache files.",
    attrs = {
        "cache_root": attr.string(doc = "Repository-relative root of the module cache."),
        "digest": attr.string(doc = "sha256 of the module manifest."),
        "ref": attr.string(mandatory = True, doc = "Module reference, br:HOST/PATH:TAG."),
        "srcs": attr.label_list(allow_files = True, doc = "Cached module files."),
    },
)
