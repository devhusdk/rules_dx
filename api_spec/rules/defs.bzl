"""OpenAPI and AsyncAPI source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

ApiSpecInfo = provider(
    doc = "The root documents and the full document closure of an api_spec_library.",
    fields = {
        "files": "Depset of File: roots, refs and the files of every dep.",
        "roots": "List of File: the root documents of this library.",
    },
)

def _api_spec_library_impl(ctx):
    files = depset(
        ctx.files.srcs + ctx.files.refs,
        transitive = [dep[ApiSpecInfo].files for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = files),
        ApiSpecInfo(files = files, roots = ctx.files.srcs),
        dx_quality_sources(ctx.files.srcs, [
            ("api_spec", ["yaml", "yml", "json"]),
            ("api_spec_ref", ["yaml", "yml", "json"], [], ctx.files.refs),
        ], str(ctx.label)),
    ]

api_spec_library = rule(
    implementation = _api_spec_library_impl,
    doc = "Owns the OpenAPI and AsyncAPI documents of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".yaml", ".yml", ".json"], doc = "Root OpenAPI or AsyncAPI documents to lint."),
        "refs": attr.label_list(allow_files = [".yaml", ".yml", ".json"], doc = "Files the roots reach through relative $ref."),
        "deps": attr.label_list(providers = [ApiSpecInfo], doc = "Spec libraries the roots reach through relative $ref."),
    },
)
