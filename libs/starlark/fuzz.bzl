"""The libFuzzer ASAN binary rule and the `<name>_run` runner shared by the fuzz macros."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def _asan_fuzzer_transition_impl(_settings, attr):
    return {
        "@llvm//config:asan": attr.asan,
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

_dx_fuzz_binary = rule(
    implementation = _dx_fuzz_binary_impl,
    attrs = {
        "binary": attr.label(doc = "Binary the runner executes.", executable = True, cfg = "target", mandatory = True),
        "asan": attr.bool(default = True, doc = "Build `binary` with AddressSanitizer."),
        "pinned_libfuzzer": attr.bool(default = True, doc = "Link the pinned LLVM's libFuzzer. Off when the binary brings its own."),
    },
    cfg = _asan_fuzzer_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's AddressSanitizer runtime when `asan` is set and, unless `pinned_libfuzzer` is off, its libFuzzer.",
)

def dx_fuzz_binary(name, binary, visibility = None, testonly = True, tags = None, target_compatible_with = None, **kwargs):
    """Declares `<name>_asan`, `<name>_noasan` and `name`, which selects `<name>_noasan` on Windows."""
    for suffix, asan in (("_asan", True), ("_noasan", False)):
        _dx_fuzz_binary(
            name = name + suffix,
            binary = binary,
            asan = asan,
            visibility = ["//visibility:private"],
            testonly = testonly,
            tags = tags,
            target_compatible_with = target_compatible_with,
            **kwargs
        )
    native.alias(
        name = name,
        actual = select({
            Label("@platforms//os:windows"): ":" + name + "_noasan",
            "//conditions:default": ":" + name + "_asan",
        }),
        visibility = visibility,
        testonly = testonly,
        tags = tags,
    )

def dx_fuzz_runner(name, fuzz, engine = "libfuzzer", target = "", harness = None, visibility = None):
    """Defines `<name>_run`, the runner `dx fuzz` calls for the fuzz binary `fuzz`."""
    sh_binary(
        name = name + "_run",
        srcs = [Label("//libs/starlark:fuzz_run.sh")],
        args = [engine, "$(rootpath {})".format(fuzz)] + ([target] if target else []) + (["$(rootpath {})".format(harness)] if harness else []),
        data = [fuzz] + ([harness] if harness else []),
        testonly = True,
        visibility = visibility,
    )
