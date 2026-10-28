"""Replays each CUDA compile action of a target up to nvcc's -cuda stage."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

NvccSyntaxInfo = provider(
    doc = "Front-end-only replays of the CudaCompile actions of one CUDA target.",
    fields = {
        "argv": "list of list of string: the original compile command line per replayed action.",
        "diagnostics": "list of File: nvcc stderr, one per replayed compile action.",
        "envs": "list of dict: the compile action environment per replayed action.",
        "inputs": "list of depset of File: the compile action inputs (plus declared headers) per replayed action.",
        "sources": "list of string: source path (execroot relative) per replayed action.",
        "statuses": "list of File: nvcc exit code, one per replayed action.",
    },
)

_MNEMONICS = ["CudaCompile"]

# Other nvcc phase flags: a command line that already selects one cannot be turned into -cuda.
_PHASES = ["-dc", "-dw", "-E", "-cuda", "-ptx", "-cubin", "-fatbin", "-dlink", "-lib", "-link", "-run", "--device-c", "--device-w", "--preprocess", "--ptx", "--cubin", "--fatbin", "--device-link", "--lib", "--link", "--run", "--cuda", "--compile"]
_OUTPUT = ["-o", "--output-file"]
_DROP = ["-MD", "-MMD", "-keep", "--keep", "-save-temps", "--save-temps", "--generate-dependencies-with-compile", "--generate-nonsystem-dependencies-with-compile"]
_DROP_WITH_VALUE = ["-MF", "-MT", "-keep-dir", "--keep-dir", "--dependency-output"]

def nvcc_syntax_argv(argv, src):
    """Rewrites an nvcc compile command line so it stops after the front end (-cuda).

    The only changes are -c -> -cuda, the output redirected to /dev/null and the removal
    of flags that write dependency files or keep temporary files. Nothing else is
    translated (no -gencode or -Xcompiler rewriting). -fdevice-syntax-only is deliberately
    never added: nvcc still exits 0 with it on a __global__ call to a __host__ function
    and on bad inline asm, so it is not a faithful typecheck.

    Pure function so other projections (editor databases) can share it.

    Args:
      argv: list of string: the compile action command line, nvcc first.
      src: string: the source path the action compiles.

    Returns:
      A struct with `argv` (the rewritten list) and `error` (non-empty when the
      command line cannot be replayed faithfully).
    """
    if len(argv) == 0:
        return struct(argv = [], error = "the compile action has an empty command line")
    if argv[0].rpartition("/")[2] != "nvcc":
        return struct(argv = [], error = "only nvcc compile commands are supported (found " + argv[0] + ")")
    out = []
    compiles = 0
    skip = False
    set_output = False
    for arg in argv:
        if skip:
            skip = False
            if set_output:
                set_output = False
                out.append("/dev/null")
            continue
        if arg.startswith("@") or arg in ["-optf", "--options-file"] or arg.startswith("--options-file="):
            return struct(argv = [], error = "params and options files are not supported (found " + arg + ")")
        if arg in _PHASES:
            return struct(argv = [], error = "the compile command line already selects another nvcc phase (found " + arg + ")")
        if arg == "-c":
            compiles += 1
            out.append("-cuda")
        elif arg in _OUTPUT:
            out.append(arg)
            skip = True
            set_output = True
        elif arg.startswith("--output-file="):
            out.append("--output-file=/dev/null")
        elif arg in _DROP_WITH_VALUE:
            skip = True
        elif arg in _DROP:
            continue
        elif arg.startswith("--dependency-output=") or arg.startswith("--keep-dir="):
            continue
        else:
            out.append(arg)
    if compiles != 1:
        return struct(argv = [], error = "expected exactly one -c in the compile command line, found " + str(compiles))
    if src not in out:
        return struct(argv = [], error = "the source " + src + " is not on the compile command line")
    return struct(argv = out, error = "")

def nvcc_syntax_source(argv):
    """Returns the translation unit of an nvcc compile command line (the argument after -c), or ""."""
    for i in range(len(argv) - 1):
        if argv[i] == "-c":
            return argv[i + 1]
    return ""

def _first(value):
    return value[0] if type(value) == "list" else value

def _compile_actions(target):
    actions = getattr(target, "actions", None)
    if actions == None:
        return []
    return [action for action in actions if action.mnemonic in _MNEMONICS]

def _nvcc_syntax_impl(target, ctx):
    attrs = ctx.rule.attr
    actions = _compile_actions(target)
    if len(actions) == 0:
        if not hasattr(attrs, "upstream"):
            return []
        upstream = _first(attrs.upstream)
        if NvccSyntaxInfo not in upstream:
            return []
        info = upstream[NvccSyntaxInfo]
        return [info, OutputGroupInfo(nvcc_syntax = depset(info.diagnostics + info.statuses))]
    if target.label.workspace_name != "":
        return []

    # Declared headers are pruned from the compile action inputs, so a replay without
    # them cannot find its own includes.
    headers = depset()
    if CcInfo in target:
        context = target[CcInfo].compilation_context
        headers = depset(context.direct_private_headers, transitive = [context.headers])
    argvs = []
    envs = []
    inputs = []
    sources = []
    diagnostics = []
    statuses = []
    for i in range(len(actions)):
        action = actions[i]
        argv = action.argv
        if argv == None:
            fail("nvcc_syntax: Bazel does not expose the command line of the %s action of %s" % (action.mnemonic, target.label))
        src = nvcc_syntax_source(list(argv))
        rewritten = nvcc_syntax_argv(list(argv), src)
        if rewritten.error != "":
            fail("nvcc_syntax: " + str(target.label) + ": " + rewritten.error)
        stem = "%s_%d" % (target.label.name, i)
        log = ctx.actions.declare_file("dx_nvcc_syntax/%s.log" % stem)
        status = ctx.actions.declare_file("dx_nvcc_syntax/%s.exit" % stem)
        all_inputs = depset(transitive = [action.inputs, headers])
        ctx.actions.run_shell(
            mnemonic = "NvccSyntax",
            progress_message = "CUDA front-end check %{label}: " + src,
            command = 'log="$1"; status="$2"; shift 2; "$@" 2> "$log"; echo "$?" > "$status"',
            arguments = [log.path, status.path] + rewritten.argv,
            inputs = all_inputs,
            outputs = [log, status],
            # nvcc.profile leaves CICC_PATH and NVVMIR_LIBRARY_DIR to the environment.
            env = action.env,
            resource_set = dx_quality_heavy_resources,
        )
        argvs.append(list(argv))
        envs.append(dict(action.env))
        inputs.append(all_inputs)
        sources.append(src)
        diagnostics.append(log)
        statuses.append(status)
    return [
        NvccSyntaxInfo(argv = argvs, envs = envs, inputs = inputs, sources = sources, diagnostics = diagnostics, statuses = statuses),
        OutputGroupInfo(nvcc_syntax = depset(diagnostics + statuses)),
    ]

nvcc_syntax_aspect = aspect(
    implementation = _nvcc_syntax_impl,
    doc = "Replays the CudaCompile actions of a CUDA target up to nvcc's -cuda stage.",
    attr_aspects = ["upstream"],
)
