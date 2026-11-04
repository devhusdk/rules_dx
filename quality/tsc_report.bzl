"""Test rule that exposes the tsc aspect output of one target."""

load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")

def _tsc_report_impl(ctx):
    info = ctx.attr.target[TscInfo]
    return [DefaultInfo(files = depset([info.stdout if ctx.attr.kind == "stdout" else info.status]))]

tsc_report = rule(
    implementation = _tsc_report_impl,
    doc = "Exposes the tsc stdout or exit status file of a target.",
    attrs = {
        "kind": attr.string(values = ["stdout", "exit"], mandatory = True),
        "target": attr.label(mandatory = True, aspects = [tsc_aspect]),
    },
)
