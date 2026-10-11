"""Subject carrying checked-in clang-tidy diagnostics for the delegated run."""

load("//quality:sources.bzl", "QualitySourcesInfo")

def _tidy_upstream_subject_impl(ctx):
    return [
        ctx.attr.target[DefaultInfo],
        QualitySourcesInfo(direct_sources = ctx.attr.target[QualitySourcesInfo].direct_sources),
        OutputGroupInfo(clang_tidy_output = depset(ctx.files.diagnostics)),
    ]

tidy_upstream_subject = rule(
    implementation = _tidy_upstream_subject_impl,
    attrs = {
        "diagnostics": attr.label_list(
            allow_files = [".txt"],
            mandatory = True,
        ),
        "target": attr.label(
            mandatory = True,
            providers = [QualitySourcesInfo],
        ),
    },
)
