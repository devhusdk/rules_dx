"""Replays each C/C++/Objective-C compile action of a target with -fsyntax-only."""

load("@apple_support//lib:apple_support.bzl", "apple_support")
load("@apple_support//xcode:providers.bzl", "XcodeVersionInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

ClangSyntaxInfo = provider(
    doc = "Syntax-only replays of the compile actions of one C/C++/Objective-C target.",
    fields = {
        "argv": "list of list of string: the original compile command line per replayed action.",
        "diagnostics": "list of File: compiler stderr, one per replayed compile action.",
        "envs": "list of dict: the compile action environment per replayed action.",
        "inputs": "list of depset of File: the compile action inputs (plus declared headers) per replayed action.",
        "sources": "list of string: source path (execroot relative) per replayed action.",
        "statuses": "list of File: compiler exit code, one per replayed action.",
        "tidy_diagnostics": "list of File: clang-tidy output, one per replayed action.",
    },
)

_SOURCE_EXTENSIONS = ["h", "hh", "hpp", "hxx", "h++", "inc", "inl", "tcc", "c", "cc", "cpp", "cxx", "c++", "m", "mm", "cu", "cuh"]
_MNEMONICS = ["CppCompile", "ObjcCompile"]

# Flags that produce build outputs and are not wanted for a syntax-only run.
_DROP = ["-MD", "-MMD", "-MG", "-MP", "-fcolor-diagnostics", "-fdiagnostics-color"]
_DROP_WITH_VALUE = ["-o", "-MF", "-MT", "-MQ", "--serialize-diagnostics"]

def clang_syntax_argv(argv, src):
    """Rewrites a compile command line into its -fsyntax-only form.

    Pure function so other projections (clang-tidy, editor databases) can share it.

    Args:
      argv: list of string: the compile action command line, compiler first.
      src: string: the source path the action compiles.

    Returns:
      A struct with `argv` (the rewritten list) and `error` (non-empty when the
      command line cannot be replayed faithfully).
    """
    if len(argv) == 0:
        return struct(argv = [], error = "the compile action has an empty command line")
    out = []
    compiles = 0
    skip = False
    for arg in argv:
        if skip:
            skip = False
            continue
        if arg == "/c" or arg.startswith("/Fo") or arg.startswith("/Fd"):
            return struct(argv = [], error = "MSVC/clang-cl command lines are not supported (found " + arg + ")")
        if arg.startswith("@"):
            return struct(argv = [], error = "params files are not supported (found " + arg + ")")
        if arg == "-c":
            compiles += 1
            out.append("-fsyntax-only")
        elif arg in _DROP_WITH_VALUE:
            skip = True
        elif arg in _DROP:
            continue
        elif arg.startswith("-MF") or arg.startswith("-MT") or arg.startswith("-MQ") or arg.startswith("-Wp,-M") or arg.startswith("--serialize-diagnostics="):
            continue
        elif arg == "-fsyntax-only":
            compiles += 1
            out.append(arg)
        else:
            out.append(arg)
    if compiles != 1:
        return struct(argv = [], error = "expected exactly one -c in the compile command line, found " + str(compiles))
    if src not in out:
        return struct(argv = [], error = "the source " + src + " is not on the compile command line")
    compiler = out[0].rpartition("/")[2]
    if "clang" in compiler:
        out += ["-fno-caret-diagnostics", "-fno-color-diagnostics"]
    elif "gcc" in compiler or "g++" in compiler or compiler in ["cc", "c++"]:
        out += ["-fno-diagnostics-show-caret", "-fdiagnostics-color=never"]
    return struct(argv = out, error = "")

def clang_tidy_argv(tidy, argv, src):
    """Projects a compile command line into a clang-tidy command line.

    Args:
      tidy: string: the clang-tidy executable path.
      argv: list of string: the compile action command line, compiler first.
      src: string: the source path the action compiles.

    Returns:
      A struct with `argv` (clang-tidy, the source, `--`, then the compile flags) and
      `error` (non-empty when the command line cannot be projected).
    """
    rewritten = clang_syntax_argv(argv, src)
    if rewritten.error != "":
        return rewritten
    flags = []
    dropped_src = False
    for arg in rewritten.argv[1:]:
        if arg == "-fsyntax-only":
            continue
        if arg == src and not dropped_src:
            dropped_src = True
            continue
        flags.append(arg)
    return struct(argv = [tidy, src, "--"] + flags, error = "")

def _first(value):
    return value[0] if type(value) == "list" else value

def _compile_actions(target):
    actions = getattr(target, "actions", None)
    if actions == None:
        return []
    return [action for action in actions if action.mnemonic in _MNEMONICS]

def clang_syntax_source(argv):
    """Returns the translation unit of a compile command line, or "" when there is none.

    A normal compile names it after -c. A header-parsing action (parse_headers) has no
    -c: it already carries -fsyntax-only and ends with the header, so the last argument
    with a C-family extension is taken.
    """
    for i in range(len(argv) - 1):
        if argv[i] == "-c":
            return argv[i + 1]
    if "-fsyntax-only" in argv:
        for arg in reversed(argv):
            if not arg.startswith("-") and arg.rpartition(".")[2] in _SOURCE_EXTENSIONS:
                return arg
    return ""

def _clang_syntax_impl(target, ctx):
    attrs = ctx.rule.attr
    actions = _compile_actions(target)
    if len(actions) == 0:
        if not hasattr(attrs, "upstream"):
            return []
        upstream = _first(attrs.upstream)
        if ClangSyntaxInfo not in upstream:
            return []
        info = upstream[ClangSyntaxInfo]
        return [info, OutputGroupInfo(clang_syntax = depset(info.diagnostics + info.statuses), clang_tidy = depset(info.tidy_diagnostics))]
    if target.label.workspace_name != "":
        return []

    xcode_config = ctx.attr._xcode_config[XcodeVersionInfo]
    # Declared headers are pruned from the compile action inputs (they are rediscovered
    # from the .d file), so a replay without them cannot find its own includes.
    headers = depset()
    if CcInfo in target:
        context = target[CcInfo].compilation_context
        headers = depset(context.direct_private_headers, transitive = [context.headers])
    tidy = ctx.file._clang_tidy
    argvs = []
    envs = []
    inputs = []
    sources = []
    diagnostics = []
    statuses = []
    tidy_diagnostics = []
    for i in range(len(actions)):
        action = actions[i]
        argv = action.argv
        if argv == None:
            fail("clang_syntax: Bazel does not expose the command line of the %s action of %s" % (action.mnemonic, target.label))
        src = clang_syntax_source(list(argv))
        rewritten = clang_syntax_argv(list(argv), src)
        if rewritten.error != "":
            fail("clang_syntax: " + str(target.label) + ": " + rewritten.error)
        stem = "%s_%d" % (target.label.name, i)
        log = ctx.actions.declare_file("dx_clang_syntax/%s.log" % stem)
        status = ctx.actions.declare_file("dx_clang_syntax/%s.exit" % stem)
        all_inputs = depset(transitive = [action.inputs, headers])
        tidy_argv = clang_tidy_argv(tidy.path, list(argv), src)
        if tidy_argv.error != "":
            fail("clang_syntax: " + str(target.label) + ": " + tidy_argv.error)
        tidy_log = ctx.actions.declare_file("dx_clang_tidy/%s.log" % stem)
        requirements = {}
        if action.mnemonic == "ObjcCompile":
            requirements = dict(xcode_config.execution_info())
        ctx.actions.run_shell(
            mnemonic = "ClangSyntax",
            progress_message = "Syntax check %{label}: " + src,
            command = 'log="$1"; status="$2"; shift 2; "$@" 2> "$log"; echo "$?" > "$status"',
            arguments = [log.path, status.path] + rewritten.argv,
            inputs = all_inputs,
            outputs = [log, status],
            env = action.env,
            execution_requirements = requirements,
            resource_set = dx_quality_heavy_resources,
        )
        ctx.actions.run_shell(
            mnemonic = "ClangTidy",
            progress_message = "clang-tidy %{label}: " + src,
            command = 'log="$1"; shift; "$@" > "$log" 2>&1; true',
            arguments = [tidy_log.path] + tidy_argv.argv,
            inputs = depset([tidy], transitive = [all_inputs]),
            outputs = [tidy_log],
            env = action.env,
            execution_requirements = requirements,
            resource_set = dx_quality_heavy_resources,
        )
        argvs.append(list(argv))
        envs.append(dict(action.env))
        inputs.append(all_inputs)
        tidy_diagnostics.append(tidy_log)
        sources.append(src)
        diagnostics.append(log)
        statuses.append(status)
    return [
        ClangSyntaxInfo(argv = argvs, envs = envs, inputs = inputs, sources = sources, diagnostics = diagnostics, statuses = statuses, tidy_diagnostics = tidy_diagnostics),
        OutputGroupInfo(clang_syntax = depset(diagnostics + statuses), clang_tidy = depset(tidy_diagnostics)),
    ]

clang_syntax_aspect = aspect(
    implementation = _clang_syntax_impl,
    doc = "Replays the compile actions of a C/C++/Objective-C target with -fsyntax-only.",
    attr_aspects = ["upstream"],
    attrs = apple_support.action_required_attrs() | {
        "_clang_tidy": attr.label(default = "@llvm//tools:clang-tidy", allow_single_file = True, cfg = "exec"),
    },
)
