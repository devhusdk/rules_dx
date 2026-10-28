"""Dialyzer PLT and tool rules over the pinned OTP runtime."""

load(":defs.bzl", "otp_tool_binary_impl")
load(":providers.bzl", "DialyzerToolInfo")

DialyzerPltInfo = provider(
    doc = "One built PLT.",
    fields = {"plt": "The .plt file."},
)

otp_tool_binary = rule(
    implementation = otp_tool_binary_impl,
    doc = "Runs `bin/<tool>` of the resolved OTP runtime.",
    executable = True,
    attrs = {"tool": attr.string(mandatory = True, doc = "The OTP `bin/` program to run.")},
    toolchains = ["@rules_erlang//tools:toolchain_type"],
)

# The PLT is built into a scratch file, then rewritten by plt_fixup.escript (see there) into $out.
_PLT_SCRIPT = """set -u
tool="$1"; escript="$2"; fixup="$3"; out="$4"; shift 4
log="$(mktemp)"
raw="$(mktemp)"
"$tool" "$@" --output_plt "$raw" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 0 ] && [ "$rc" -ne 2 ]; then
    cat "$log" >&2
    exit 1
fi
"$escript" "$fixup" "$raw" "$out" || exit 1
rm -f "$log" "$raw"
"""

def dialyzer_plt_action(ctx, dialyzer, escript, fixup, apps, plts, beams, out):
    """Builds one PLT of OTP `apps` and `beams`, layered over `plts`.

    Dialyzer exits 2 when it only emitted warnings, which a PLT build does for
    unresolved calls; only exit 1 is a failure, and then its output is printed.

    Args:
      ctx: The rule context.
      dialyzer: The dialyzer launcher's FilesToRunProvider.
      escript: The escript launcher's FilesToRunProvider.
      fixup: The plt_fixup.escript file.
      apps: OTP application names to include.
      plts: PLT files the new PLT builds on.
      beams: .beam files to include.
      out: The declared output PLT.
    """
    args = ctx.actions.args()
    args.add(dialyzer.executable)
    args.add(escript.executable)
    args.add(fixup)
    args.add(out)
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
    ctx.actions.run_shell(
        mnemonic = "DxDialyzerPlt",
        progress_message = "Building dialyzer PLT %{output}",
        command = _PLT_SCRIPT,
        arguments = [args],
        inputs = list(plts) + list(beams) + [fixup],
        outputs = [out],
        tools = [dialyzer, escript],
        use_default_shell_env = False,
    )

def _dialyzer_plt_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".plt")
    base = [dep[DialyzerPltInfo].plt for dep in ctx.attr.plts]
    dialyzer_plt_action(ctx, ctx.attr.dialyzer[DefaultInfo].files_to_run, ctx.attr.escript[DefaultInfo].files_to_run, ctx.file._fixup, ctx.attr.apps, base, [], out)
    return [DefaultInfo(files = depset([out])), DialyzerPltInfo(plt = out)]

dialyzer_plt = rule(
    implementation = _dialyzer_plt_impl,
    doc = "A PLT of OTP applications, layered over other PLTs.",
    attrs = {
        "apps": attr.string_list(mandatory = True, doc = "OTP application names."),
        "dialyzer": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "An otp_tool_binary for dialyzer."),
        "escript": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "An otp_tool_binary for escript."),
        "plts": attr.label_list(providers = [DialyzerPltInfo], doc = "PLTs this one builds on, in order."),
        "_fixup": attr.label(default = "//erlang/rules:plt_fixup.escript", allow_single_file = True),
    },
    provides = [DefaultInfo, DialyzerPltInfo],
)

def _dialyzer_tool_impl(ctx):
    binary = ctx.attr.binary[DefaultInfo]
    plts = [dep[DialyzerPltInfo].plt for dep in ctx.attr.plts]
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = launcher, target_file = binary.files_to_run.executable, is_executable = True)
    return [
        DefaultInfo(
            executable = launcher,
            files = depset([launcher]),
            runfiles = binary.default_runfiles,
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
