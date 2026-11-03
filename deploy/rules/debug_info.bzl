"""Splits Linux ELF debug info from a binary."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("//libs/starlark:wrapper.bzl", "dx_toolchain_tool")

def _split_impl(ctx):
    cc_toolchain = find_cc_toolchain(ctx, mandatory = False)
    objcopy = dx_toolchain_tool(cc_toolchain, "llvm-objcopy") or ctx.executable._llvm_objcopy
    stripped = ctx.actions.declare_file(ctx.label.name)
    debug = ctx.actions.declare_file(ctx.label.name + ".debug")
    manifest = ctx.actions.declare_file(ctx.label.name + ".symbols.json")
    ctx.actions.run(
        executable = ctx.executable._split,
        arguments = [
            "--objcopy",
            objcopy.path,
            "--in",
            ctx.executable.binary.path,
            "--stripped",
            stripped.path,
            "--debug",
            debug.path,
            "--manifest",
            manifest.path,
        ],
        inputs = [ctx.executable.binary],
        tools = [objcopy],
        outputs = [stripped, debug, manifest],
        mnemonic = "DxSplitDebugInfo",
        progress_message = "Splitting debug info of %{label}",
    )
    return [
        DefaultInfo(files = depset([stripped]), executable = stripped),
        OutputGroupInfo(symbols = depset([debug, manifest])),
    ]

_split = rule(
    implementation = _split_impl,
    doc = "Writes a stripped binary, a debug file and a symbols manifest.",
    attrs = {
        "binary": attr.label(
            doc = "The cc_binary to split.",
            executable = True,
            cfg = "target",
            mandatory = True,
        ),
        "_llvm_objcopy": attr.label(
            default = Label("@llvm//tools:llvm-objcopy"),
            executable = True,
            allow_files = True,
            cfg = "exec",
        ),
        "_split": attr.label(
            default = Label("//deploy/symbols:split"),
            executable = True,
            cfg = "exec",
        ),
    },
    toolchains = use_cc_toolchain(mandatory = False),
    fragments = ["cpp"],
    executable = True,
)

def dx_split_debug_info(name, binary, **kwargs):
    """Splits the debug info of a Linux cc_binary; build the symbols output group to get it."""
    kwargs.setdefault("target_compatible_with", ["@platforms//os:linux"])
    _split(name = name, binary = binary, **kwargs)
