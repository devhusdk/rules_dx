"""Replays the compile actions of a Fortran, D, Haskell, OCaml or Zig target in a check-only form.

The aspect reads the real upstream compile action (flags, include paths, module
directories, compiler), rewrites its command line so nothing is produced, and runs it in
a sandboxed action that records stderr and the exit code. The runner only parses them.
"""

load("//fortran/private/toolchain:flags.bzl", "MODULE_FLAG_FORMAT")
load("//core:resources.bzl", "dx_heavy_resources")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:real_core.bzl", "real_context")

CompileCheckInfo = provider(
    doc = "Check-only replays of the compile actions of one target.",
    fields = {
        "diagnostics": "list of File: compiler stderr, one per replayed compile action.",
        "scratch": "list of File: the directory each replayed Haskell action wrote its .hie files to; empty for other tools.",
        "sources": "list of string: the first source path (execroot relative) per replayed action.",
        "statuses": "list of File: compiler exit code, one per replayed action.",
        "tool": "string: the quality tool id the logs belong to (flang, ghc, ldc, ocaml or zig).",
    },
)

_MODULE_PREFIX = MODULE_FLAG_FORMAT.partition("{}")[0]

_LDC_DROP_PREFIXES = ["-of", "-od", "-op", "-H", "-X", "-D", "-deps", "-makedeps", "-mixin", "-ftime-trace", "-cov"]
_LDC_DROP = ["-c", "-lib", "-shared", "-oq", "-cov"]

def _fail(message):
    return struct(argv = [], steps = [], error = message, source = "", staged = [])

def flang_check_argv(argv, scratch):
    """Rewrites a flang compile command line into its -fsyntax-only form."""
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
    """Rewrites an ldc/ldmd2 compile command line so it type-checks and writes nothing."""
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
    """Rewrites a zig build-exe/build-lib/test command line so it type-checks and emits nothing."""
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

_GHC_FLAGS = ["-fno-code", "-fdiagnostics-as-json", "-fwrite-ide-info"]

_GHC_DIRS = ["-odir", "-hidir", "-stubdir", "-hiedir", "-dumpdir"]

def ghc_check_argv(argv, scratch):
    """Rewrites a rules_haskell ghc_wrapper command line into a check-only replay."""
    if len(argv) != 3:
        return _fail("expected the ghc wrapper with two flag files, found %d arguments" % len(argv))
    if argv[2].startswith("@"):
        return _fail("the persistent worker is not supported (found " + argv[2] + ")")
    if scratch == "":
        return _fail("the Haskell replay needs a scratch directory")
    options = ["--append", "2", scratch + "/extra_args"]
    for flag in _GHC_FLAGS:
        options += ["--line", flag]
    for flag in _GHC_DIRS:
        options += ["--line", flag, "--line", scratch]
    return struct(argv = argv, options = options, error = "", source = "")

_OCAML_DROP = ["-bin-annot", "-annot", "-c"]

_OCAML_VALUE_FLAGS = ["-w", "-warn-error"]

_OCAML_FLAGS = ["-stop-after", "typing", "-error-style", "short", "-color", "never"]

def _basename(path):
    return path.rpartition("/")[2]

def _stem(path):
    return _basename(path).rpartition(".")[0]

def ocaml_check_argv(argv, scratch):
    """Rewrites an ocamlc or ocamlopt compile command line into check-only steps."""
    if len(argv) == 0:
        return _fail("the compile action has an empty command line")
    common = []
    positional = []
    impl = ""
    intf = ""
    output = ""
    compiles = 0
    take = ""
    previous = ""
    for arg in argv[1:]:
        value_of = previous
        previous = arg
        if take != "":
            if take == "-o":
                output = arg
            elif take == "-impl":
                impl = arg
            elif take == "-intf":
                intf = arg
            take = ""
            continue
        if arg.startswith("@") and value_of not in _OCAML_VALUE_FLAGS:
            return _fail("params files are not supported (found " + arg + ")")
        if arg in ["-o", "-impl", "-intf"]:
            take = arg
        elif arg == "-c":
            compiles += 1
        elif arg in _OCAML_DROP:
            continue
        elif arg.endswith(".ml") or arg.endswith(".mli"):
            positional.append(arg)
        else:
            common.append(arg)
    if take != "":
        return _fail("the compile action ends with " + take)
    if compiles != 1:
        return _fail("expected exactly one -c in the compile action")
    if scratch == "":
        return _fail("the OCaml replay needs a scratch directory")
    head = [argv[0]] + _OCAML_FLAGS
    ext = ".cmx" if "ocamlopt" in _basename(argv[0]) else ".cmo"
    if impl != "" and intf == "" and len(positional) == 0 and output != "":
        steps = [head + common + ["-c", "-impl", impl, "-o", scratch + "/" + _basename(output)]]
        staged = [impl]
    elif intf != "" and impl == "" and len(positional) == 0 and output != "":
        steps = [head + common + ["-c", "-intf", intf, "-o", scratch + "/" + _stem(output) + ".cmi"]]
        staged = [intf]
    elif impl == "" and intf == "" and output == "" and len(positional) == 2 and positional[0].endswith(".mli") and positional[1].endswith(".ml"):
        mli = positional[0]
        ml = positional[1]
        module = _stem(ml)
        if _stem(mli) != module:
            return _fail("the signature " + mli + " and the implementation " + ml + " name different modules")
        steps = [
            head + common + ["-c", "-intf", mli, "-o", scratch + "/" + module + ".cmi"],
            head + ["-I", scratch] + common + ["-c", "-impl", ml, "-o", scratch + "/" + module + ext],
        ]
        staged = [mli, ml]
    else:
        return _fail("the compile action has an unsupported shape (sources: %s, -impl: %s, -intf: %s, -o: %s)" % (positional, impl, intf, output))
    return struct(argv = steps[0], steps = steps, error = "", source = staged[-1], staged = staged)

def ocaml_aliases(staged, owned):
    """Maps staged source paths in compiler messages back to the files a target owns."""
    out = []
    for path in staged:
        if path in owned:
            continue
        for candidate in owned:
            if candidate.endswith(".mli") == path.endswith(".mli"):
                out.append((path, candidate))
                break
    return out

def zig_replayable(argv):
    """Whether a Zig action compiles a root module; relink actions (e.g. bc to .a) do not."""
    for arg in argv or []:
        if arg.startswith("-M") and "=" in arg and arg.endswith(".zig"):
            return True
        if not arg.startswith("-") and arg.endswith(".zig"):
            return True
    return False

_SPECS = {
    "CompileOCamlExecModule": struct(tool = "ocaml", rewrite = ocaml_check_argv, scratch = True),
    "CompileOCamlModule": struct(tool = "ocaml", rewrite = ocaml_check_argv, scratch = True),
    "CompileOcamlSignature": struct(tool = "ocaml", rewrite = ocaml_check_argv, scratch = True),
    "Dcompile": struct(tool = "ldc", rewrite = ldc_check_argv, scratch = False),
    "FortranCompile": struct(tool = "flang", rewrite = flang_check_argv, scratch = True),
    "HaskellBuildBinary": struct(tool = "ghc", rewrite = ghc_check_argv, scratch = True),
    "HaskellBuildLibrary": struct(tool = "ghc", rewrite = ghc_check_argv, scratch = True),
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
    if target.label.repo_name != "":
        return []
    attrs = ctx.rule.attr

    # A ppx module compiles generated code, so a position in it is not a position in the source.
    ppx = getattr(attrs, "ppx", None) != None
    actions = [a for a in getattr(target, "actions", []) if a.mnemonic in _SPECS and (_SPECS[a.mnemonic].tool != "zig" or zig_replayable(a.argv)) and not (ppx and _SPECS[a.mnemonic].tool == "ocaml")]
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
    scratch_dirs = []
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
            if tool == "ghc":
                scratch_dirs.append(scratch_dir)
        rewritten = spec.rewrite(list(argv), scratch)
        if rewritten.error != "":
            fail("compile_check: " + str(target.label) + ": " + rewritten.error)
        log = ctx.actions.declare_file("dx_compile_check/%s.log" % stem)
        status = ctx.actions.declare_file("dx_compile_check/%s.exit" % stem)
        steps = getattr(rewritten, "steps", None) or [rewritten.argv]
        command = [log.path, status.path] + getattr(rewritten, "options", [])
        source = rewritten.source
        if spec.tool == "ghc":
            srcs = [f.path for f in ctx.rule.files.srcs if f.path.endswith((".hs", ".lhs"))]
            if len(srcs) == 0:
                fail("compile_check: " + str(target.label) + ": no .hs or .lhs source")
            source = srcs[0]
        if spec.tool == "ocaml":
            files = ctx.rule.files
            owned = [f.path for f in getattr(files, "struct", []) + getattr(files, "sig", []) + getattr(files, "src", []) if f.path.endswith(".ml") or f.path.endswith(".mli")]
            for pair in ocaml_aliases(rewritten.staged, owned):
                command += ["--alias", pair[0], pair[1]]
        command.append("--")
        for n in range(len(steps)):
            if n > 0:
                command.append("--dx-next--")
            command += steps[n]
        ctx.actions.run(
            mnemonic = "CompileCheck",
            progress_message = "Check %{label}: " + source,
            executable = ctx.executable._compile_check_exec,
            arguments = command,
            inputs = action.inputs,
            outputs = outputs + [log, status],
            env = action.env,
            execution_requirements = getattr(action, "execution_info", None),
            resource_set = dx_heavy_resources,
        )
        diagnostics.append(log)
        statuses.append(status)
        sources.append(source)
    return _forwarded(CompileCheckInfo(diagnostics = diagnostics, sources = sources, scratch = scratch_dirs, statuses = statuses, tool = tool))

compile_check_aspect = aspect(
    implementation = _compile_check_impl,
    doc = "Replays the Fortran, D, Haskell, OCaml and Zig compile actions of a target without producing outputs.",
    attr_aspects = ["upstream", "deps"],
    attrs = {
        "_compile_check_exec": attr.label(
            default = "//quality:compile_check_exec.sh",
            allow_single_file = True,
            cfg = "exec",
            executable = True,
        ),
    },
)

def compile_check_hook(tool):
    """Returns the context hook that hands the replay logs of the tool to the runner."""
    def _hook(target, _ctx, resolved, _capability, _what, _shared):
        if CompileCheckInfo not in target or target[CompileCheckInfo].tool != tool:
            return without(resolved, tool)
        info = target[CompileCheckInfo]
        tool_files = []
        inputs = []
        for i in range(len(info.diagnostics)):
            inputs.append(info.diagnostics[i])
            stem = info.diagnostics[i].basename.rpartition(".")[0]
            stage(tool_files, inputs, "dx-compile-check/" + stem + "/exit", info.statuses[i])
        return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

    return _hook
