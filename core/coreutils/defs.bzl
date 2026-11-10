"""The pinned coreutils toolchain: one multicall binary and a directory of applets for PATH."""

COREUTILS_TOOLCHAIN_TYPE = Label("//core/coreutils:toolchain_type")

COREUTILS_APPLETS = [
    "basename",
    "cat",
    "cp",
    "dirname",
    "env",
    "head",
    "ln",
    "mkdir",
    "mktemp",
    "mv",
    "readlink",
    "realpath",
    "rm",
    "sort",
    "tail",
    "touch",
    "tr",
    "uname",
    "wc",
]

def applet_file_name(applet, binary_name):
    """Returns the file name of one applet beside a multicall binary of the given name."""
    return applet + ".exe" if binary_name.endswith(".exe") else applet

def _coreutils_toolchain_impl(ctx):
    binary = ctx.file.binary
    applets = []
    for applet in COREUTILS_APPLETS:
        link = ctx.actions.declare_file(ctx.label.name + "_bin/" + applet_file_name(applet, binary.basename))
        ctx.actions.symlink(output = link, target_file = binary, is_executable = True)
        applets.append(link)
    return [platform_common.ToolchainInfo(
        coreutils = struct(
            binary = binary,
            files = depset(applets + [binary]),
            path = applets[0].dirname,
        ),
    )]

coreutils_toolchain = rule(
    implementation = _coreutils_toolchain_impl,
    doc = "One pinned coreutils multicall binary and its applet directory.",
    attrs = {
        "binary": attr.label(mandatory = True, allow_single_file = True, doc = "The multicall binary."),
    },
)

def coreutils(ctx):
    """Returns the resolved coreutils: `binary`, `files` to add as inputs, and `path` for PATH."""
    return ctx.toolchains[COREUTILS_TOOLCHAIN_TYPE].coreutils

def _coreutils_applet_test_impl(ctx):
    tools = coreutils(ctx)
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run(
        mnemonic = "DxCoreutilsApplet",
        executable = tools.path + "/" + applet_file_name("touch", tools.binary.basename),
        arguments = [out.path],
        inputs = tools.files,
        outputs = [out],
        env = {"PATH": tools.path},
    )
    return [DefaultInfo(files = depset([out]))]

coreutils_applet_probe = rule(
    implementation = _coreutils_applet_test_impl,
    doc = "Runs the pinned `touch` applet by name with only the applet directory on PATH.",
    toolchains = [COREUTILS_TOOLCHAIN_TYPE],
)
