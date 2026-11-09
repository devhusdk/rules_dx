"""The libFuzzer ASAN binary rule and the `<name>_run` runner shared by the fuzz macros."""

load(":fuzz_bench.bzl", "merge_tags", "split_runner_kwargs")

def _asan_fuzzer_transition_impl(_settings, attr):
    return {
        "@llvm//config:asan": True,
        "@llvm//config:fuzzer": attr.pinned_libfuzzer,
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
        "pinned_libfuzzer": attr.bool(default = True, doc = "Link the pinned LLVM's libFuzzer. Off when the binary brings its own."),
    },
    cfg = _asan_fuzzer_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's AddressSanitizer runtime and, unless `pinned_libfuzzer` is off, its libFuzzer.",
)

def _dx_fuzz_run_impl(ctx):
    runner = ctx.attr.runner[DefaultInfo].files_to_run.executable
    executable = ctx.actions.declare_file(ctx.label.name + (".exe" if runner.extension == "exe" else ""))
    ctx.actions.symlink(output = executable, target_file = runner, is_executable = True)
    runfiles = ctx.runfiles(files = [executable] + ctx.files.data).merge(ctx.attr.runner[DefaultInfo].default_runfiles)
    for data in ctx.attr.data:
        runfiles = runfiles.merge(data[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = executable, runfiles = runfiles)]

_dx_fuzz_run = rule(
    implementation = _dx_fuzz_run_impl,
    attrs = {
        "data": attr.label_list(allow_files = True),
        "runner": attr.label(default = Label("//libs/fuzz_run:fuzz_run"), executable = True, cfg = "target"),
    },
    executable = True,
)

def dx_fuzz_runner(name, fuzz, engine = "libfuzzer", target = "", harness = None, visibility = None):
    """Defines `<name>_run`, the runner `dx fuzz` calls for the fuzz binary `fuzz`."""
    _dx_fuzz_run(
        name = name + "_run",
        args = [engine, "$(rootpath {})".format(fuzz)] + ([target] if target else []) + (["$(rootpath {})".format(harness)] if harness else []),
        data = [fuzz] + ([harness] if harness else []),
        testonly = True,
        visibility = visibility,
    )

def dx_fuzz_test(name, binary_rule, binary_kwargs, tags = None, visibility = None, pinned_libfuzzer = True, **kwargs):
    """Declares the private `<name>_bin` with `binary_rule`, the `dx_fuzz_binary` `<name>` and the `<name>_run` runner.

    `binary_kwargs` are the language rule's own attributes. `kwargs` split into those the
    binary takes and the runner-facing `args` and `target_compatible_with`.
    """
    build, run = split_runner_kwargs(kwargs)
    binary_rule(
        name = name + "_bin",
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
        **(binary_kwargs | build)
    )
    dx_fuzz_binary(
        name = name,
        binary = ":" + name + "_bin",
        pinned_libfuzzer = pinned_libfuzzer,
        visibility = visibility,
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        **run
    )
    dx_fuzz_runner(name, ":" + name, visibility = visibility)
