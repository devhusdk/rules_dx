"""The libFuzzer binary rule and the `<name>_run` runner shared by the fuzz macros."""

load(":fuzz_bench.bzl", "merge_tags", "split_runner_kwargs", "split_test_kwargs", "with_default_size")

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

DxFuzzBinaryInfo = provider(
    doc = "The sanitizer settings a `dx_fuzz_binary` applies to its binary.",
    fields = {"asan": "Whether AddressSanitizer is on.", "fuzzer": "Whether the pinned libFuzzer is linked."},
)

def _dx_fuzz_binary_impl(ctx):
    binary = ctx.attr.binary
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = binary[DefaultInfo].files_to_run.executable, is_executable = True)
    runfiles = ctx.runfiles(files = [executable]).merge(binary[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = executable, runfiles = runfiles),
        DxFuzzBinaryInfo(asan = ctx.attr.asan, fuzzer = ctx.attr.pinned_libfuzzer),
    ]

dx_fuzz_binary = rule(
    implementation = _dx_fuzz_binary_impl,
    attrs = {
        "binary": attr.label(doc = "Binary the runner executes.", executable = True, cfg = "target", mandatory = True),
        "asan": attr.bool(default = True, doc = "Build `binary` with AddressSanitizer."),
        "pinned_libfuzzer": attr.bool(default = True, doc = "Link the pinned LLVM's libFuzzer. Off when the binary brings its own."),
    },
    cfg = _asan_fuzzer_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's AddressSanitizer runtime when `asan` is on and, unless `pinned_libfuzzer` is off, its libFuzzer.",
)

def dx_fuzz_binary_select(name, *, tags = None, visibility = None, testonly = True, target_compatible_with = None, **kwargs):
    """Declares `<name>` as a `dx_fuzz_binary` with AddressSanitizer everywhere except Windows, where it is off and the pinned libFuzzer config is not applied."""
    inner_tags = merge_tags([tag for tag in (tags or []) if tag != "dx-fuzz"], "manual")
    noasan_kwargs = dict(kwargs)
    noasan_kwargs["pinned_libfuzzer"] = False
    dx_fuzz_binary(
        name = name + "_asan",
        asan = True,
        testonly = testonly,
        tags = inner_tags,
        target_compatible_with = target_compatible_with,
        visibility = ["//visibility:private"],
        **kwargs
    )
    dx_fuzz_binary(
        name = name + "_noasan",
        asan = False,
        testonly = testonly,
        tags = inner_tags,
        target_compatible_with = target_compatible_with,
        visibility = ["//visibility:private"],
        **noasan_kwargs
    )
    native.alias(
        name = name,
        actual = select({
            Label("@platforms//os:windows"): ":" + name + "_noasan",
            "//conditions:default": ":" + name + "_asan",
        }),
        testonly = testonly,
        tags = tags,
        target_compatible_with = target_compatible_with,
        visibility = visibility,
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
    doc = "The runner executable that `dx fuzz` calls for one fuzz binary.",
    attrs = {
        "data": attr.label_list(doc = "Files the runner needs at run time.", allow_files = True),
        "runner": attr.label(doc = "The fuzz_run binary.", default = Label("//cli/fuzz_run:fuzz_run"), executable = True, cfg = "target"),
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

def dx_fuzz_target(name, rule, *, tags = None, visibility = None, engine = "libfuzzer", target = "", fuzz = None, harness = None, **attrs):
    """Declares the fuzz binary `<name>` with `rule` and the `<name>_run` runner `dx fuzz` calls."""
    rule(
        name = name,
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        visibility = visibility,
        **attrs
    )
    dx_fuzz_runner(name, fuzz or ":" + name, engine = engine, target = target, harness = harness, visibility = visibility)

def dx_fuzz_test(
        name,
        binary_rule,
        binary_kwargs,
        tags = None,
        visibility = None,
        pinned_libfuzzer = True,
        fuzz_rule = None,
        fuzz_kwargs = None,
        engine = "libfuzzer",
        target = "",
        test = False,
        **kwargs):
    """Declares the private `<name>_bin` with `binary_rule`, the fuzz binary `<name>` and the `<name>_run` runner."""
    if test:
        build, run = split_test_kwargs(kwargs)
        run = with_default_size(run)
    else:
        build, run = split_runner_kwargs(kwargs)
    binary_rule(
        name = name + "_bin",
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
        **(build | binary_kwargs)
    )
    if fuzz_rule == None:
        fuzz_rule = dx_fuzz_binary_select
        fuzz_kwargs = {"binary": ":" + name + "_bin", "pinned_libfuzzer": pinned_libfuzzer}
    dx_fuzz_target(name, fuzz_rule, tags = tags, visibility = visibility, engine = engine, target = target, **(run | (fuzz_kwargs or {})))
