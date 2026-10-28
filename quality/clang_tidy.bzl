"""Runs clang-tidy over the compile actions a target's ClangSyntaxInfo recorded."""

load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

_DEFAULT_CHECKS = "clang-diagnostic-*,clang-analyzer-*"

def clang_tidy_argv(argv, src, config_path = ""):
    """Projects a -fsyntax-only compile command line onto a clang-tidy command line.

    Pure function: the compiler is dropped, the translation unit becomes clang-tidy's
    positional argument and everything else is the fixed compilation database entry.

    Args:
      argv: list of string: the rewritten compile command line, compiler first.
      src: string: the translation unit.
      config_path: string: the .clang-tidy file to pass, or "" for the default checks.

    Returns:
      A struct with `argv` (arguments for clang-tidy) and `error` (non-empty when
      the command line is not a clang one).
    """
    if len(argv) == 0:
        return struct(argv = [], error = "the compile action has an empty command line")
    compiler = argv[0].rpartition("/")[2]
    if "clang" not in compiler:
        return struct(argv = [], error = "clang-tidy needs a clang compile command line, found " + compiler + " (use a clang cc toolchain)")
    rest = list(argv[1:])
    if src not in rest:
        return struct(argv = [], error = "the source " + src + " is not on the compile command line")
    last = len(rest) - 1 - rest[::-1].index(src)
    rest = rest[:last] + rest[last + 1:]
    out = ["--quiet"]
    if config_path != "":
        out.append("--config-file=" + config_path)
    else:
        # clang-tidy 22 enables no check by default and the workspace's .clang-tidy
        # is not an input of the action; these were the long-standing defaults.
        out.append("--checks=" + _DEFAULT_CHECKS)
    out += [src, "--"] + rest
    return struct(argv = out, error = "")

def clang_tidy_indices(sources, owned):
    """Returns the indices of the translation units a clang_tidy stage owns.

    Args:
      sources: list of string: the translation unit of each recorded compile action.
      owned: list of string or None: the stage sources; None keeps everything.

    Returns:
      list of int. Generated files (bazel-out paths) and files of a language family the
      policy does not lint are left out, since their findings could not be attributed.
    """
    if owned == None:
        return list(range(len(sources)))
    return [i for i in range(len(sources)) if sources[i] in owned]

# Bazel's Apple wrapper expands the placeholders in argv; clang-tidy is run without it.
_SCRIPT = """
log="$1"; err="$2"; status="$3"; tidy="$4"; shift 4
n=$#
while [ "$n" -gt 0 ]; do
  a="$1"; shift
  a="${a//__BAZEL_XCODE_DEVELOPER_DIR__/$DEVELOPER_DIR}"
  a="${a//__BAZEL_XCODE_SDKROOT__/$SDKROOT}"
  a="${a//__BAZEL_EXECUTION_ROOT__/$PWD}"
  set -- "$@" "$a"
  n=$((n - 1))
done
raw="$(mktemp)"
"$tidy" "$@" > "$raw" 2> "$err"
echo "$?" > "$status"
# clang-tidy prints absolute paths, source excerpts and notes; keep the located
# warning and error lines, relative to the execroot.
awk -v p="$PWD/" 'index($0, p) == 1 { $0 = substr($0, length(p) + 1) }
  /^[^ ].*:[0-9]+:[0-9]+: (warning|error): / { print }' "$raw" > "$log"
rm -f "$raw"
"""

def clang_tidy_actions(ctx, label, info, binary, config = None, config_inputs = [], owned = None):
    """Declares one clang-tidy action per recorded compile action.

    Args:
      ctx: the aspect context.
      label: Label of the analysed target.
      info: ClangSyntaxInfo of the target.
      binary: File: the clang-tidy executable.
      config: File or None: the resolved .clang-tidy.
      config_inputs: list of File: the config and everything it includes.
      owned: list of string or None: the sources the clang_tidy stages own. Compile
        actions of any other translation unit (a generated file, a language family the
        policy does not lint) get no action, since the runner could not attribute
        their findings.

    Returns:
      A struct of parallel lists: `logs` (stdout), `errs` (stderr) and `statuses`.
    """
    for src, message in info.errors.items():
        if src == "" or owned == None or src in owned:
            fail("clang_tidy: %s: %s" % (label, message))
    logs = []
    errs = []
    statuses = []
    for i in clang_tidy_indices(info.sources, owned):
        src = info.sources[i]
        projected = clang_tidy_argv(info.argvs[i], src, config.path if config != None else "")
        if projected.error != "":
            fail("clang_tidy: " + str(label) + ": " + projected.error)
        stem = "%s_%d" % (label.name, i)
        log = ctx.actions.declare_file("dx_clang_tidy/%s.log" % stem)
        err = ctx.actions.declare_file("dx_clang_tidy/%s.err" % stem)
        status = ctx.actions.declare_file("dx_clang_tidy/%s.exit" % stem)
        ctx.actions.run_shell(
            mnemonic = "ClangTidy",
            progress_message = "clang-tidy %{label}: " + src,
            command = _SCRIPT,
            arguments = [log.path, err.path, status.path, binary.path] + projected.argv,
            inputs = depset([binary] + config_inputs, transitive = [info.inputs[i]]),
            outputs = [log, err, status],
            env = info.envs[i],
            execution_requirements = info.execution_requirements[i],
            resource_set = dx_quality_heavy_resources,
        )
        logs.append(log)
        errs.append(err)
        statuses.append(status)
    return struct(logs = logs, errs = errs, statuses = statuses)
