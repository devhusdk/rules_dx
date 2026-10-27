"""SQL source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

_EXTENSIONS = ["sql"]

def _sql_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("sql", _EXTENSIONS)], str(ctx.label)),
    ]

sql_library = rule(
    implementation = _sql_library_impl,
    doc = "Owns the SQL files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = ["." + ext for ext in _EXTENSIONS], doc = "SQL files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "SQL libraries this package depends on."),
    },
)
