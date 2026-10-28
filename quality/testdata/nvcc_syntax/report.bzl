"""Exposes the nvcc_syntax logs and exit codes of a target as plain files."""

load("//quality:nvcc_syntax.bzl", "NvccSyntaxInfo", "nvcc_syntax_aspect")

def _report_impl(ctx):
    info = ctx.attr.target[NvccSyntaxInfo]
    return [DefaultInfo(files = depset(info.diagnostics + info.statuses))]

nvcc_syntax_report = rule(
    implementation = _report_impl,
    attrs = {"target": attr.label(aspects = [nvcc_syntax_aspect], mandatory = True)},
)
