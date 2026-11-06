"""Test rule that exposes the tsc aspect output of one target."""

load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")

def _tsc_report_impl(ctx):
    info = ctx.attr.target[TscInfo]
    if ctx.attr.kind == "exit":
        return [DefaultInfo(files = depset([info.status]))]
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    # run_shell: A pipe and a redirect normalize the output.
    ctx.actions.run_shell(
        inputs = [info.stdout],
        outputs = [out],
        command = "tr -d '\\r' < \"$1\" | tr '\\\\' / > \"$2\"",
        arguments = [info.stdout.path, out.path],
        mnemonic = "TscReportNormalize",
    )
    return [DefaultInfo(files = depset([out]))]

tsc_report = rule(
    implementation = _tsc_report_impl,
    doc = "Exposes the tsc stdout, with CRs removed and backslashes turned into slashes, or the exit status file of a target.",
    attrs = {
        "kind": attr.string(values = ["stdout", "exit"], mandatory = True),
        "target": attr.label(mandatory = True, aspects = [tsc_aspect]),
    },
)
