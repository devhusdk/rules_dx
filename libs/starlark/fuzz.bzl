"""The libFuzzer ASAN binary rule and the `<name>_run` runner shared by the fuzz macros."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def _asan_fuzzer_transition_impl(_settings, _attr):
    return {
        "@llvm//config:asan": True,
        "@llvm//config:fuzzer": True,
    }

_asan_fuzzer_transition = transition(
    implementation = _asan_fuzzer_transition_impl,
    inputs = [],
    outputs = ["@llvm//config:asan", "@llvm//config:fuzzer"],
)

def _dx_fuzz_binary_impl(ctx):
    binary = ctx.attr.binary
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = binary[DefaultInfo].files_to_run.executable, is_executable = True)
    runfiles = ctx.runfiles(files = [executable]).merge(binary[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = executable, runfiles = runfiles)]

dx_fuzz_binary = rule(
    implementation = _dx_fuzz_binary_impl,
    attrs = {
        "binary": attr.label(doc = "Binary the runner executes.", executable = True, cfg = "target", mandatory = True),
    },
    cfg = _asan_fuzzer_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's AddressSanitizer and libFuzzer runtimes.",
)

def dx_fuzz_runner(name, fuzz, engine = "libfuzzer", target = "", visibility = None):
    """Defines `<name>_run`, the runner `dx fuzz` calls for the fuzz binary `fuzz`.

    Args:
      name: the fuzz target; the runner is `<name>_run`.
      fuzz: label of the fuzz binary or test.
      engine: `libfuzzer`, `jazzer` or `go`.
      target: the Jazzer target class or the Go fuzz function; unused for libfuzzer.
      visibility: visibility of the runner.
    """
    sh_binary(
        name = name + "_run",
        srcs = [Label("//libs/starlark:fuzz_run.sh")],
        args = [engine, "$(rootpath {})".format(fuzz), target],
        data = [fuzz],
        testonly = True,
        visibility = visibility,
    )
