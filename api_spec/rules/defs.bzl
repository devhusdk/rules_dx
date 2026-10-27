"""OpenAPI and AsyncAPI source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _api_spec_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs + ctx.files.refs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("api_spec", ["yaml", "yml", "json"])], str(ctx.label)),
    ]

api_spec_library = rule(
    implementation = _api_spec_library_impl,
    doc = "Owns the OpenAPI and AsyncAPI documents of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".yaml", ".yml", ".json"], doc = "Root OpenAPI or AsyncAPI documents to lint."),
        "refs": attr.label_list(allow_files = [".yaml", ".yml", ".json"], doc = "Files the roots reach through relative $ref."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Spec libraries the roots reach through relative $ref."),
    },
)
