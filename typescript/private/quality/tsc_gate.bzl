"""Fails the build when the dx tsc run over a ts_project exits non-zero."""

load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load(":tsc.bzl", "TscInfo", "bin_relative", "tsc_aspect")

def _tsc_gate_impl(ctx):
    info = ctx.attr.project[TscInfo]
    marker = ctx.actions.declare_file(ctx.label.name + ".ok")
    args = ctx.actions.args()
    args.add(bin_relative(ctx, marker))
    for status, log in zip(info.statuses, info.logs):
        args.add(bin_relative(ctx, status))
        args.add(bin_relative(ctx, log))
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "TscGate",
        progress_message = "tsc gate %{label}",
        executable = ctx.executable._gate,
        inputs = depset(info.statuses + info.logs),
        outputs = [marker],
        arguments = [args],
    )
    return [DefaultInfo(files = depset([marker]))]

tsc_gate = rule(
    implementation = _tsc_gate_impl,
    doc = "Builds only when tsc --noEmit over project exits 0.",
    attrs = {
        "project": attr.label(aspects = [tsc_aspect], mandatory = True, doc = "The ts_project target to typecheck."),
        "_gate": attr.label(
            default = "//typescript/private/quality:tsc_gate",
            executable = True,
            cfg = "exec",
        ),
    },
)
