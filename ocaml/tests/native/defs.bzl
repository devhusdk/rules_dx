"""Test-only rules that build a target under the native OCaml emitter."""

load("//libs/starlark:wrapper.bzl", "dx_lcov_merger_attr")

_EMITTER = str(Label("//ocaml/toolchain:emitter"))

def _sys_impl(_settings, _attr):
    return {_EMITTER: "sys"}

_sys = transition(
    implementation = _sys_impl,
    inputs = [],
    outputs = [_EMITTER],
)

def _sys_cell_test_impl(ctx):
    target = ctx.attr.target[0]
    info = target[DefaultInfo]
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = info.files_to_run.executable, is_executable = True)
    return [
        DefaultInfo(
            executable = executable,
            runfiles = ctx.runfiles([executable, info.files_to_run.executable]).merge(info.default_runfiles),
        ),
        coverage_common.instrumented_files_info(ctx, dependency_attributes = ["target"]),
        target[RunEnvironmentInfo] if RunEnvironmentInfo in target else RunEnvironmentInfo(),
    ]

sys_cell_test = rule(
    implementation = _sys_cell_test_impl,
    doc = "Runs one ocaml_test built with the native (sys) emitter.",
    test = True,
    attrs = dx_lcov_merger_attr() | {
        "target": attr.label(mandatory = True, cfg = _sys, executable = True, doc = "The ocaml_test to build natively."),
    },
)
