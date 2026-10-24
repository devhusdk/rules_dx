"""YAML source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _yaml_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("yaml", ["yaml", "yml"])], str(ctx.label)),
    ]

yaml_library = rule(
    implementation = _yaml_library_impl,
    doc = "Owns the YAML files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".yaml", ".yml"], doc = "YAML files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "YAML libraries this package depends on."),
    },
)
