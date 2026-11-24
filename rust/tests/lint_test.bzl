"""Clippy as a test: fails when clippy reports a finding on a Rust target."""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("//core:rlocation.bzl", "rlocation_path")
load("//core:wrapper.bzl", "dx_symlink_executable_name")
load("//rust/private/quality:rust_clippy.bzl", "DxClippyInfo", "dx_clippy_aspect")

_DIAGNOSTICS_ENV = "DX_CLIPPY_DIAGNOSTICS"

def _lint_impl(ctx):
    diagnostics = []
    for target in ctx.attr.targets:
        found = target[DxClippyInfo].diagnostics
        if not found:
            fail("%s: clippy does not lint this target" % target.label)
        diagnostics.extend(found)
    checker = ctx.executable._checker
    executable = ctx.actions.declare_file(dx_symlink_executable_name(ctx.label.name, checker))
    ctx.actions.symlink(output = executable, target_file = checker, is_executable = True)
    runfiles = ctx.runfiles(files = [executable] + diagnostics).merge(ctx.attr._checker[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = executable, runfiles = runfiles),
        RunEnvironmentInfo({_DIAGNOSTICS_ENV: " ".join([rlocation_path(ctx, f) for f in diagnostics])}),
        OutputGroupInfo(clippy_diagnostics = depset(diagnostics)),
    ]

_ATTRS = {
    "_checker": attr.label(
        default = Label("//rust/private/tools/clippy_findings"),
        executable = True,
        cfg = "target",
    ),
    "targets": attr.label_list(
        doc = "Rust crates and tests to lint.",
        allow_empty = False,
        aspects = [dx_clippy_aspect],
        mandatory = True,
        providers = [[rust_common.crate_info], [rust_common.test_crate_info]],
    ),
}

lint_test = rule(
    implementation = _lint_impl,
    doc = "Runs clippy with the build's clippy flags on targets and fails on any finding.",
    test = True,
    attrs = _ATTRS,
)

lint_binary = rule(
    implementation = _lint_impl,
    doc = "The lint_test check as a non-test executable.",
    executable = True,
    attrs = _ATTRS,
)
