"""Dialyzer PLT and tool rules over the pinned OTP runtime."""

load("//libs/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "native_launch_descriptor_path")
load(":defs.bzl", "otp_tool_binary_impl")
load(":providers.bzl", "DialyzerToolInfo", _DialyzerPltInfo = "DialyzerPltInfo")

DialyzerPltInfo = _DialyzerPltInfo

otp_tool_binary = rule(
    implementation = otp_tool_binary_impl,
    doc = "Runs `bin/<tool>` of the resolved OTP runtime.",
    executable = True,
    attrs = dict(NATIVE_LAUNCHER_ATTRS, tool = attr.string(mandatory = True, doc = "The OTP `bin/` program to run.")),
    toolchains = ["@rules_erlang//tools:toolchain_type"],
)

def _dirs(files):
    """The distinct directories of `files`, in order."""
    seen = {}
    for f in files:
        seen[f.dirname] = True
    return list(seen)

def dialyzer_plt_action(ctx, dialyzer, escript, driver, apps, plts, beams, code, out):
    """Builds one PLT of OTP `apps` and `beams`, layered over `plts`."""
    args = ctx.actions.args()
    args.add(driver)
    args.add(dialyzer.executable)
    args.add(out)
    for directory in _dirs(code):
        args.add("-pa")
        args.add(directory)
    args.add("--build_plt")
    if plts:
        args.add("--plts")
        args.add_all(plts)
    args.add("--no_check_plt")
    if apps:
        args.add("--apps")
        args.add_all(apps)
    if beams:
        args.add("--")
        args.add_all(beams)
    ctx.actions.run(
        mnemonic = "DxDialyzerPlt",
        progress_message = "Building dialyzer PLT %{output}",
        executable = escript,
        arguments = [args],
        inputs = list(plts) + list(beams) + list(code) + [driver],
        outputs = [out],
        tools = [dialyzer],
        # The Windows erlexec stops with "No PATH variable (!)" when PATH is unset, and a hermetic
        # action has none. The value is never searched for a tool, so it names no real directory.
        env = {"PATH": "/dx_no_path"},
        use_default_shell_env = False,
    )

def _dialyzer_plt_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".plt")
    base = [dep[DialyzerPltInfo].plt for dep in ctx.attr.plts]
    if not ctx.attr.apps and not ctx.files.beams:
        fail("dialyzer_plt " + ctx.label.name + ": set apps or beams")
    dialyzer_plt_action(ctx, ctx.attr.dialyzer[DefaultInfo].files_to_run, ctx.attr.escript[DefaultInfo].files_to_run, ctx.file._driver, ctx.attr.apps, base, ctx.files.beams, ctx.files.code, out)
    return [DefaultInfo(files = depset([out])), DialyzerPltInfo(plt = out)]

dialyzer_plt = rule(
    implementation = _dialyzer_plt_impl,
    doc = "A PLT of OTP applications and beams, layered over other PLTs.",
    attrs = {
        "apps": attr.string_list(doc = "OTP application names."),
        "beams": attr.label_list(allow_files = [".beam"], doc = "Extra .beam files to include."),
        "code": attr.label_list(allow_files = [".beam"], doc = "Beams whose directories go on dialyzer's code path, to read the debug info of `beams`."),
        "dialyzer": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "An otp_tool_binary for dialyzer."),
        "escript": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "An otp_tool_binary for escript."),
        "plts": attr.label_list(providers = [DialyzerPltInfo], doc = "PLTs this one builds on, in order."),
        "_driver": attr.label(default = "//erlang/private:plt_build.escript", allow_single_file = True),
    },
    provides = [DefaultInfo, DialyzerPltInfo],
)

def tool_launcher(ctx, binary_target):
    """Links an otp_tool_binary as this rule's executable."""
    binary = binary_target[DefaultInfo]
    exe = binary.files_to_run.executable
    runfiles = binary.default_runfiles
    if exe.path.endswith(".exe"):
        launcher = ctx.actions.declare_file(ctx.label.name + ".exe")
        ctx.actions.symlink(output = launcher, target_file = exe, is_executable = True)
        wanted = native_launch_descriptor_path(exe.path)
        descriptors = [f for f in binary.default_runfiles.files.to_list() if f.path == wanted]
        if not descriptors:
            fail(str(ctx.label) + ": " + wanted + " is not in the runfiles of " + str(binary_target.label))
        descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
        ctx.actions.symlink(output = descriptor, target_file = descriptors[0])
        runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(binary.default_runfiles)
    else:
        launcher = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.symlink(output = launcher, target_file = exe, is_executable = True)
    return launcher, runfiles

def _dialyzer_tool_impl(ctx):
    plts = [dep[DialyzerPltInfo].plt for dep in ctx.attr.plts]
    launcher, runfiles = tool_launcher(ctx, ctx.attr.binary)
    return [
        DefaultInfo(
            executable = launcher,
            files = depset([launcher]),
            runfiles = runfiles,
        ),
        DialyzerToolInfo(plts = plts),
    ]

dialyzer_tool = rule(
    implementation = _dialyzer_tool_impl,
    doc = "The dialyzer launcher plus the PLTs `dx check` runs it against.",
    executable = True,
    attrs = {
        "binary": attr.label(mandatory = True, executable = True, cfg = "target", doc = "An otp_tool_binary for dialyzer."),
        "plts": attr.label_list(mandatory = True, providers = [DialyzerPltInfo], doc = "PLTs in analysis order."),
    },
    provides = [DefaultInfo, DialyzerToolInfo],
)
