"""Exposes the dx lint result files of source targets to tests."""

load("//core/quality:families/core.bzl", "real_lint_aspect", "real_vale_lint_aspect")
load("//core/quality:families/text.bzl", "real_text_lint_aspect")

visibility(["//tools/docs/site"])

def _lint_results_impl(ctx):
    results = [
        src[OutputGroupInfo].dx_results
        for src in ctx.attr.srcs
        if OutputGroupInfo in src and hasattr(src[OutputGroupInfo], "dx_results")
    ]
    files = depset(transitive = results)
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

lint_results = rule(
    implementation = _lint_results_impl,
    doc = "Runs the prose lint aspects dx lint runs on the srcs and exposes their result files.",
    attrs = {
        "srcs": attr.label_list(
            doc = "Source targets whose prose is linted.",
            mandatory = True,
            aspects = [real_lint_aspect, real_text_lint_aspect, real_vale_lint_aspect],
        ),
    },
)
