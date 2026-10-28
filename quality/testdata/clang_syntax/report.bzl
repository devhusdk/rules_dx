"""Exposes the clang_syntax logs and exit codes of a target as plain files."""

load("//quality:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")

def _report_impl(ctx):
    info = ctx.attr.target[ClangSyntaxInfo]
    return [DefaultInfo(files = depset(info.diagnostics + info.statuses))]

clang_syntax_report = rule(
    implementation = _report_impl,
    attrs = {"target": attr.label(aspects = [clang_syntax_aspect], mandatory = True)},
)
