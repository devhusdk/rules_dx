"""Splits Linux ELF debug info from a binary and pairs macOS dSYM bundles and Windows PDBs with it."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("//core:wrapper.bzl", "dx_toolchain_tool")

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

_DSYM_FLAG = "//command_line_option:apple_generate_dsym"

def _dsym_transition_impl(_settings, _attr):
    return {_DSYM_FLAG: True}

_dsym_transition = transition(
    implementation = _dsym_transition_impl,
    inputs = [],
    outputs = [_DSYM_FLAG],
)

def _dsym_pair_impl(ctx):
    binary_target = ctx.attr.binary[0]
    binary = ctx.executable.binary
    dsyms = getattr(binary_target[OutputGroupInfo], "dsyms", None)
    if dsyms == None or len(dsyms.to_list()) != 1:
        fail("%s did not produce one dSYM bundle; the dx_dsym binary must be a macOS cc_binary" % binary_target.label)
    bundle = dsyms.to_list()[0]
    dsymutil = ctx.executable._dsymutil
    manifest = ctx.actions.declare_file(ctx.label.name + ".dsym.json")
    ctx.actions.run(
        executable = ctx.executable._dsym,
        arguments = [
            "--dsymutil",
            dsymutil.path,
            "--in",
            binary.path,
            "--bundle",
            bundle.path,
            "--manifest",
            manifest.path,
        ],
        inputs = [binary, bundle],
        tools = [dsymutil],
        outputs = [manifest],
        mnemonic = "DxDsym",
        progress_message = "Pairing the dSYM of %{label}",
    )
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = binary, is_executable = True)
    return [
        DefaultInfo(
            files = depset([executable]),
            executable = executable,
            runfiles = binary_target[DefaultInfo].default_runfiles,
        ),
        OutputGroupInfo(symbols = depset([bundle, manifest])),
    ]

_dsym_pair = rule(
    implementation = _dsym_pair_impl,
    doc = "Pairs a macOS cc_binary with the dSYM its link wrote; the dSYM stays in the symbols group.",
    attrs = {
        "binary": attr.label(
            doc = "The cc_binary whose dSYM is kept. Built with apple_generate_dsym.",
            executable = True,
            cfg = _dsym_transition,
            mandatory = True,
        ),
        "_dsymutil": attr.label(
            default = Label("@llvm//tools:dsymutil"),
            allow_single_file = True,
            executable = True,
            cfg = "exec",
        ),
        "_dsym": attr.label(
            default = Label("//deploy/symbols:dsym"),
            executable = True,
            cfg = "exec",
        ),
        "_allowlist_function_transition": attr.label(
            default = Label("@bazel_tools//tools/allowlists/function_transition_allowlist"),
        ),
    },
    executable = True,
)

def dx_dsym(name, binary, **kwargs):
    """Keeps the dSYM of a macOS cc_binary, checked against it; build the symbols output group to get it."""
    kwargs.setdefault("target_compatible_with", ["@platforms//os:macos"])
    _dsym_pair(name = name, binary = binary, **kwargs)

_FEATURES_FLAG = "//command_line_option:features"

def _pdb_transition_impl(settings, _attr):
    features = list(settings[_FEATURES_FLAG])
    if "generate_pdb_file" not in features:
        features.append("generate_pdb_file")
    return {_FEATURES_FLAG: features}

_pdb_transition = transition(
    implementation = _pdb_transition_impl,
    inputs = [_FEATURES_FLAG],
    outputs = [_FEATURES_FLAG],
)

def _pdb_pair_impl(ctx):
    binary = ctx.executable.binary
    target = ctx.attr.binary[0]
    pdbs = getattr(target[OutputGroupInfo], "pdb_file", None)
    if pdbs == None or len(pdbs.to_list()) != 1:
        fail("%s did not produce one pdb_file; the dx_pdb binary must be a Windows cc_binary" % target.label)
    pdb = pdbs.to_list()[0]
    manifest = ctx.actions.declare_file(ctx.label.name + ".pdb.json")
    exe = ctx.actions.declare_file(ctx.label.name + ".exe")
    ctx.actions.symlink(output = exe, target_file = binary, is_executable = True)
    ctx.actions.run(
        executable = ctx.executable._pdb,
        arguments = [
            "--in",
            binary.path,
            "--pdb",
            pdb.path,
            "--manifest",
            manifest.path,
        ],
        inputs = [binary, pdb],
        outputs = [manifest],
        mnemonic = "DxPdb",
        progress_message = "Pairing the PDB of %{label}",
    )
    return [
        DefaultInfo(
            executable = exe,
            files = depset([exe]),
            runfiles = target[DefaultInfo].default_runfiles,
        ),
        OutputGroupInfo(symbols = depset([pdb, manifest])),
    ]

_pdb_pair = rule(
    implementation = _pdb_pair_impl,
    doc = "Pairs a Windows cc_binary with the PDB its link wrote; the PDB stays in the symbols group.",
    attrs = {
        "binary": attr.label(
            doc = "The cc_binary whose PDB is kept. Built with the generate_pdb_file feature.",
            executable = True,
            cfg = _pdb_transition,
            mandatory = True,
        ),
        "_pdb": attr.label(
            default = Label("//deploy/symbols:pdb"),
            executable = True,
            cfg = "exec",
        ),
        "_allowlist_function_transition": attr.label(
            default = Label("@bazel_tools//tools/allowlists/function_transition_allowlist"),
        ),
    },
    executable = True,
)

def dx_pdb(name, binary, **kwargs):
    """Keeps the PDB of a Windows cc_binary, checked against it; build the symbols output group to get it."""
    kwargs.setdefault("target_compatible_with", ["@platforms//os:windows"])
    _pdb_pair(name = name, binary = binary, **kwargs)
