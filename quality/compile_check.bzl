"""Replays the compile actions of a Fortran, D or Zig target in a check-only form.

The aspect reads the real upstream compile action (flags, include paths, module
directories, compiler), rewrites its command line so nothing is produced, and runs it in
a sandboxed action that records stderr and the exit code. The runner only parses them.
"""

load("//fortran/toolchain:toolchain.bzl", "MODULE_FLAG_FORMAT")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

CompileCheckInfo = provider(
    doc = "Check-only replays of the compile actions of one target.",
    fields = {
        "diagnostics": "list of File: compiler stderr, one per replayed compile action.",
        "sources": "list of string: the first source path (execroot relative) per replayed action.",
        "statuses": "list of File: compiler exit code, one per replayed action.",
        "tool": "string: the quality tool id the logs belong to (flang, ldc or zig).",
    },
)

_MODULE_PREFIX = MODULE_FLAG_FORMAT.partition("{}")[0]

_LDC_DROP_PREFIXES = ["-of", "-od", "-op", "-H", "-X", "-D", "-deps", "-makedeps", "-mixin", "-ftime-trace", "-cov"]
_LDC_DROP = ["-c", "-lib", "-shared", "-oq", "-cov"]

def _fail(message):
    return struct(argv = [], error = message, source = "")

def flang_check_argv(argv, scratch):
    """Rewrites a flang compile command line into its -fsyntax-only form.

    Args:
      argv: list of string: the compile action command line, compiler first.
      scratch: string: a directory the replay may write module files to.

    Returns:
      A struct with `argv`, `error` (non-empty when the command cannot be replayed
      faithfully) and `source` (the translation unit).
    """
    if len(argv) == 0:
        return _fail("the compile action has an empty command line")
    out = []
    compiles = 0
    source = ""
    modules = 0
    skip = False
    take_source = False
    for arg in argv:
        if skip:
            skip = False
            continue
        if arg.startswith("@"):
            return _fail("params files are not supported (found " + arg + ")")
        if take_source:
            take_source = False
            source = arg
            out.append(arg)
        elif arg == "-c":
            compiles += 1
            take_source = True
            out.append("-fsyntax-only")
        elif arg == "-o":
            skip = True
        elif arg.startswith(_MODULE_PREFIX) and len(arg) > len(_MODULE_PREFIX):
            modules += 1
            out.append(_MODULE_PREFIX + scratch)
        else:
            out.append(arg)
    if compiles != 1:
        return _fail("expected exactly one -c in the compile command line, found " + str(compiles))
    if source == "" or source.startswith("-"):
        return _fail("the compile command line names no source after -c")
    if modules == 0:
        out.append(_MODULE_PREFIX + scratch)
    return struct(argv = out, error = "", source = source)

def ldc_check_argv(argv, scratch):
    """Rewrites an ldc/ldmd2 compile command line so it type-checks and writes nothing.

    Args:
      argv: list of string: the compile action command line, compiler first.
      scratch: string: unused; kept so every family rewrite has one signature.

    Returns:
      A struct with `argv`, `error` and `source` (the first D source).
    """
    _ = scratch
    if len(argv) == 0:
        return _fail("the compile action has an empty command line")
    out = [argv[0]]
    source = ""
    for arg in argv[1:]:
        if arg.startswith("@"):
            return _fail("params files are not supported (found " + arg + ")")
        if arg in _LDC_DROP:
            continue
        if arg.startswith("-") and arg != "-o-" and [p for p in _LDC_DROP_PREFIXES if arg.startswith(p)] and not arg.startswith("-debug") and not arg.startswith("-defaultlib") and not arg.startswith("-dip"):
            continue
        if not arg.startswith("-") and arg.endswith((".d", ".di")) and source == "":
            source = arg
        out.append(arg)
    if source == "":
        return _fail("the compile command line names no D source")
    return struct(argv = out + ["-o-", "-verror-style=gnu", "-vcolumns"], error = "", source = source)

def zig_check_argv(argv, scratch):
    """Rewrites a zig build-exe/build-lib/test command line so it type-checks and emits nothing.

    Every `-femit-*` flag is dropped and `-fno-emit-bin` is appended. Zig analyzes lazily, so only
    declarations reachable from the root module are checked.

    Args:
      argv: list of string: the compile action command line, compiler first.
      scratch: string: unused; kept so every family rewrite has one signature.

    Returns:
      A struct with `argv`, `error` and `source` (the root module source: the first module listed).
    """
    _ = scratch
    if len(argv) < 2:
        return _fail("the compile action has no zig subcommand")
    out = []
    source = ""
    has_no_emit = False
    for arg in argv:
        if arg.startswith("@"):
            return _fail("params files are not supported (found " + arg + ")")
        if arg == "-fno-emit-bin":
            has_no_emit = True
        elif arg.startswith("-femit-"):
            continue
        elif arg.startswith("-M") and "=" in arg and arg.endswith(".zig"):
            if source == "":
                source = arg.partition("=")[2]
        elif not arg.startswith("-") and arg.endswith(".zig") and source == "":
            source = arg
        out.append(arg)
    if source == "":
        return _fail("the compile command line names no Zig source")
    if not has_no_emit:
        out.append("-fno-emit-bin")
    return struct(argv = out, error = "", source = source)

_SPECS = {
    "Dcompile": struct(tool = "ldc", rewrite = ldc_check_argv, scratch = False),
    "FortranCompile": struct(tool = "flang", rewrite = flang_check_argv, scratch = True),
    "ZigBuildExe": struct(tool = "zig", rewrite = zig_check_argv, scratch = False),
    "ZigBuildLib": struct(tool = "zig", rewrite = zig_check_argv, scratch = False),
    "ZigBuildSharedLib": struct(tool = "zig", rewrite = zig_check_argv, scratch = False),
    "ZigBuildStaticLib": struct(tool = "zig", rewrite = zig_check_argv, scratch = False),
    "ZigBuildTest": struct(tool = "zig", rewrite = zig_check_argv, scratch = False),
}

# The quality tool ids whose logs the replay produces, derived from _SPECS.
COMPILE_CHECK_TOOLS = sorted({spec.tool: True for spec in _SPECS.values()}.keys())

def _first(value):
    return value[0] if type(value) == "list" else value

def _forwarded(info):
    return [info, OutputGroupInfo(compile_check = depset(info.diagnostics + info.statuses))]

def _compile_check_impl(target, ctx):
    if target.label.workspace_name != "":
        return []
    actions = [a for a in getattr(target, "actions", []) if a.mnemonic in _SPECS]
    attrs = ctx.rule.attr
    if len(actions) == 0:
        upstream = None
        if hasattr(attrs, "upstream"):
            upstream = _first(attrs.upstream)
        elif ctx.rule.kind in ["cc_binary", "cc_test"]:
            for dep in getattr(attrs, "deps", []):
                if dep.label.name == target.label.name + "_fortran" and CompileCheckInfo in dep:
                    upstream = dep
        if upstream == None or CompileCheckInfo not in upstream:
            return []
        return _forwarded(upstream[CompileCheckInfo])

    tool = _SPECS[actions[0].mnemonic].tool
    diagnostics = []
    statuses = []
    sources = []
    for i in range(len(actions)):
        action = actions[i]
        spec = _SPECS[action.mnemonic]
        argv = action.argv
        if argv == None:
            fail("compile_check: Bazel does not expose the command line of the %s action of %s" % (action.mnemonic, target.label))
        stem = "%s_%d" % (target.label.name, i)
        outputs = []
        scratch = ""
        if spec.scratch:
            scratch_dir = ctx.actions.declare_directory("dx_compile_check/%s.scratch" % stem)
            outputs.append(scratch_dir)
            scratch = scratch_dir.path
        rewritten = spec.rewrite(list(argv), scratch)
        if rewritten.error != "":
            fail("compile_check: " + str(target.label) + ": " + rewritten.error)
        log = ctx.actions.declare_file("dx_compile_check/%s.log" % stem)
        status = ctx.actions.declare_file("dx_compile_check/%s.exit" % stem)
        ctx.actions.run_shell(
            mnemonic = "CompileCheck",
            progress_message = "Check %{label}: " + rewritten.source,
            command = 'log="$1"; status="$2"; shift 2; "$@" 2> "$log"; echo "$?" > "$status"',
            arguments = [log.path, status.path] + rewritten.argv,
            inputs = action.inputs,
            outputs = outputs + [log, status],
            env = action.env,
            execution_requirements = getattr(action, "execution_info", None),
            resource_set = dx_quality_heavy_resources,
        )
        diagnostics.append(log)
        statuses.append(status)
        sources.append(rewritten.source)
    return _forwarded(CompileCheckInfo(diagnostics = diagnostics, sources = sources, statuses = statuses, tool = tool))

compile_check_aspect = aspect(
    implementation = _compile_check_impl,
    doc = "Replays the Fortran, D and Zig compile actions of a target without producing outputs.",
    attr_aspects = ["upstream", "deps"],
)
