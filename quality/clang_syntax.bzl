"""Replays each C/C++/Objective-C compile action of a target with -fsyntax-only."""

load("@apple_support//lib:apple_support.bzl", "apple_support")
load("@apple_support//xcode:providers.bzl", "XcodeVersionInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:resources.bzl", "dx_heavy_resources")
load("//cc/toolchains:bindings.bzl", "CC_TOOLCHAIN_TYPE", "cc_toolchain_identity")
load("//quality:native_config.bzl", "DxNativeConfigInfo", "bound_config_targets")
load("//quality:nvcc_syntax.bzl", "nvcc_syntax_select", "nvcc_syntax_source")
load("//quality:policy.bzl", "QualityPolicyInfo")

ClangSyntaxInfo = provider(
    doc = "Syntax-only replays of the compile actions of one C/C++/Objective-C target.",
    fields = {
        "argv": "list of list of string: the original compile command line per replayed action.",
        "diagnostics": "list of File: compiler stderr, one per replayed compile action.",
        "drivers": "list of File: the compiler driver style (gnu, msvc or unknown), one per replayed action.",
        "envs": "list of dict: the compile action environment per replayed action.",
        "inputs": "list of depset of File: the compile action inputs (plus declared headers) per replayed action.",
        "sources": "list of string: source path (execroot relative) per replayed action.",
        "statuses": "list of File: compiler exit code, one per replayed action.",
        "tidy_diagnostics": "list of File: clang-tidy output, one per replayed action.",
    },
)

_SOURCE_EXTENSIONS = ["h", "hh", "hpp", "hxx", "h++", "inc", "inl", "tcc", "c", "cc", "cpp", "cxx", "c++", "m", "mm", "cu", "cuh"]
_MNEMONICS = ["CppCompile", "ObjcCompile", "CudaCompile"]
_CUDA_VALUE_FLAGS = ["-I", "-isystem", "-iquote", "-D", "-U", "-include"]
_CUDA_PASS_THROUGH = ["-Xcompiler", "--compiler-options", "-Xcudafe"]

# Flags that produce build outputs and are not wanted for a syntax-only run.
_DROP = ["-MD", "-MMD", "-MG", "-MP", "-fcolor-diagnostics", "-fdiagnostics-color"]
_DROP_WITH_VALUE = ["-o", "-MF", "-MT", "-MQ", "--serialize-diagnostics"]
_GCC_ONLY = ["-fno-canonical-system-headers", "-fcanonical-system-headers", "-fno-diagnostics-show-caret", "-fdiagnostics-color=never"]
# bash on Windows rewrites /I-style arguments and INCLUDE-style variables as paths.
_MSYS_NO_CONVERSION = {"MSYS_NO_PATHCONV": "1", "MSYS2_ARG_CONV_EXCL": "*", "MSYS2_ENV_CONV_EXCL": "*"}
_MSVC_DROP_PREFIXES = ["Fo", "Fd", "Fe", "Fa", "Fp", "Fi", "showIncludes", "sourceDependencies"]

def _msvc_dropped(arg):
    return len(arg) > 1 and arg[0] in "/-" and arg[1:].startswith(tuple(_MSVC_DROP_PREFIXES))

def _msvc_syntax_argv(argv, src, identity):
    out = []
    compiles = 0
    skip = False
    for i in range(len(argv)):
        arg = argv[i]
        if skip:
            skip = False
            continue
        if arg.startswith("@"):
            return struct(argv = [], error = "response files in copts are not supported (found " + arg + ")")
        if arg in ["/c", "-c"]:
            compiles += 1
            out.append(identity.syntax_only)
        elif arg in ["/sourceDependencies", "-sourceDependencies"]:
            skip = True
        elif _msvc_dropped(arg):
            continue
        else:
            out.append(arg)
    if compiles != 1:
        return struct(argv = [], error = "expected exactly one /c in the compile command line, found " + str(compiles))
    if src not in out:
        return struct(argv = [], error = "the source " + src + " is not on the compile command line")
    return struct(argv = out + ["/diagnostics:column"], error = "")

def clang_syntax_argv(argv, src, identity):
    """Rewrites a compile command line into its -fsyntax-only form.

    Pure function so other projections (clang-tidy, editor databases) can share it.

    Args:
      argv: list of string: the compile action command line, compiler first.
      src: string: the source path the action compiles.
      identity: struct: the compiler identity from cc_compiler_identity.

    Returns:
      A struct with `argv` (the rewritten list) and `error` (non-empty when the
      command line cannot be replayed faithfully).
    """
    if len(argv) == 0:
        return struct(argv = [], error = "the compile action has an empty command line")
    if identity.driver == "msvc":
        return _msvc_syntax_argv(argv, src, identity)
    out = []
    compiles = 0
    skip = False
    for arg in argv:
        if skip:
            skip = False
            continue
        if arg.startswith("@"):
            return struct(argv = [], error = "response files in copts are not supported (found " + arg + ")")
        if arg == "-c":
            compiles += 1
            out.append("-fsyntax-only")
            if identity.family == "gcc":
                out.append("-c")
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
    if identity.family == "clang":
        out += ["-fno-caret-diagnostics", "-fno-color-diagnostics"]
    elif identity.family == "gcc":
        out += ["-fno-diagnostics-show-caret", "-fdiagnostics-color=never"]
    return struct(argv = out, error = "")

def _config_flag(config):
    return ["--config-file=" + config] if config else []

_MSVC_TIDY_UNSUPPORTED = {
    "Yc": "precompiled headers",
    "Yu": "precompiled headers",
    "clr": "C++/CLI",
    "ZW": "C++/CX",
}

def _msvc_tidy_argv(tidy, argv, src, identity, config):
    rewritten = clang_syntax_argv(argv, src, identity)
    if rewritten.error != "":
        return rewritten
    for arg in argv[1:]:
        if len(arg) > 1 and arg[0] in "/-":
            for prefix, capability in _MSVC_TIDY_UNSUPPORTED.items():
                if arg[1:].startswith(prefix):
                    return struct(argv = [], error = "clang-tidy does not support %s (flag %s)" % (capability, arg))
    flags = []
    dropped_src = False
    for arg in rewritten.argv[1:-1]:
        if arg == identity.syntax_only:
            continue
        if arg == src and not dropped_src:
            dropped_src = True
            continue
        flags.append(arg)
    extra = ["--extra-arg-before=--driver-mode=cl"]
    if identity.target:
        extra.append("--extra-arg-before=--target=" + identity.target)
    return struct(argv = [tidy] + _config_flag(config) + extra + [src, "--"] + flags, error = "")

def clang_tidy_argv(tidy, argv, src, identity, config = ""):
    """Projects a compile command line into a clang-tidy command line.

    Args:
      tidy: string: the clang-tidy executable path.
      argv: list of string: the compile action command line, compiler first.
      src: string: the source path the action compiles.
      identity: struct: the compiler identity from cc_compiler_identity.
      config: string: the .clang-tidy path, or empty for none.

    Returns:
      A struct with `argv` (clang-tidy, the source, `--`, then the compile flags) and
      `error` (non-empty when the command line cannot be projected).
    """
    if identity.driver == "msvc":
        return _msvc_tidy_argv(tidy, argv, src, identity, config)
    rewritten = clang_syntax_argv(argv, src, identity)
    if rewritten.error != "":
        return rewritten
    flags = []
    dropped_src = False
    for arg in rewritten.argv[1:]:
        if arg == "-fsyntax-only":
            continue
        if identity.family == "gcc" and arg in _GCC_ONLY:
            continue
        if arg == src and not dropped_src:
            dropped_src = True
            continue
        flags.append(arg)
    return struct(argv = [tidy] + _config_flag(config) + [src, "--"] + flags, error = "")

def cuda_tidy_argv(tidy, argv, src, config = ""):
    """Projects an nvcc compile command line into a clang-tidy command line (`-x cuda`).

    Only the flags clang understands the same way are carried over (include paths,
    defines, -std, -include); nvcc-only flags (-gencode, -arch, -Xcompiler wrappers,
    -rdc, ...) are dropped. --cuda-path is the toolkit root derived from the nvcc path.

    Args:
      tidy: string: the clang-tidy executable path.
      argv: list of string: the nvcc compile action command line, nvcc first.
      src: string: the source path the action compiles.
      config: string: the .clang-tidy path, or empty for none.

    Returns:
      A struct with `argv` and `error` (non-empty when the command line cannot be projected).
    """
    if len(argv) == 0 or argv[0].replace("\\", "/").rpartition("/")[2] not in ["nvcc", "nvcc.exe"]:
        return struct(argv = [], error = "only nvcc compile commands can be projected into clang-tidy")
    if src == "" or src not in argv:
        return struct(argv = [], error = "the source " + src + " is not on the compile command line")
    root = argv[0].replace("\\", "/").rpartition("/")[0].rpartition("/")[0]
    flags = ["-x", "cuda", "--cuda-host-only", "-nocudalib", "-Wno-unknown-cuda-version"]
    if root != "":
        flags.append("--cuda-path=" + root)
    i = 1
    for _ in range(len(argv)):
        if i >= len(argv):
            break
        arg = argv[i]
        i += 1
        if arg in _CUDA_VALUE_FLAGS and i < len(argv):
            flags += [arg, argv[i]]
            i += 1
        elif arg in ["-Xcompiler", "--compiler-options"] and i < len(argv):
            # Host compiler flags: only the ones clang shares survive.
            for part in argv[i].split(","):
                if part.startswith("/"):
                    return struct(argv = [], error = "clang-tidy does not support nvcc with an MSVC host compiler (flag %s)" % part)
                if part.startswith(("-I", "-D", "-U", "-isystem", "-std=", "-iquote")):
                    flags.append(part)
            i += 1
        elif arg in _CUDA_PASS_THROUGH and i < len(argv):
            i += 1
        elif arg.startswith(("-I", "-D", "-U", "-isystem", "-iquote")) or arg.startswith(("-std=", "--std=")):
            flags.append(arg.replace("--std=", "-std=", 1))
        elif arg in ["-ccbin", "--compiler-bindir"] and i < len(argv):
            host = argv[i].replace("\\", "/").rpartition("/")[2].lower()
            if host in ["cl", "cl.exe", "clang-cl", "clang-cl.exe"]:
                return struct(argv = [], error = "clang-tidy does not support nvcc with an MSVC host compiler (flag %s %s)" % (arg, argv[i]))
            i += 1
        elif arg in ["-o", "-MF", "-MT", "-arch", "-code", "-gencode", "--output-file"] and i < len(argv):
            i += 1
    return struct(argv = [tidy] + _config_flag(config) + [src, "--"] + flags, error = "")

def _failing_tidy(ctx, log, src, error):
    # run_shell: The shell redirects output or runs the bash runner script.
    ctx.actions.run_shell(
        mnemonic = "ClangTidy",
        progress_message = "clang-tidy %{label}: " + src,
        command = 'echo "$2" > "$1"; echo "$2" >&2; exit 1',
        arguments = [log.path, error],
        outputs = [log],
    )

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
        if argv[i] in ["-c", "/c"]:
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
        return [info, OutputGroupInfo(clang_syntax = depset(info.diagnostics + info.statuses + info.drivers), clang_tidy = depset(info.tidy_diagnostics))]
    if target.label.repo_name != "":
        return []

    identity = cc_toolchain_identity(ctx)
    xcode_config = ctx.attr._xcode_config[XcodeVersionInfo]
    # Declared headers are pruned from the compile action inputs (they are rediscovered
    # from the .d file), so a replay without them cannot find its own includes.
    headers = depset()
    if CcInfo in target:
        context = target[CcInfo].compilation_context
        headers = depset(context.direct_private_headers, transitive = [context.headers])
    tidy = ctx.file._clang_tidy
    tidy_config = ctx.file._clang_tidy_default.path
    tidy_closure = depset([ctx.file._clang_tidy_default])
    tidy_target = bound_config_targets(attrs, ctx.attr._policy[QualityPolicyInfo].configs, str(target.label)).get("clang_tidy")
    if tidy_target:
        tidy_config = tidy_target[DxNativeConfigInfo].config.path
        tidy_closure = tidy_target[DxNativeConfigInfo].closure
    runner = ctx.file._clang_tidy_run
    argvs = []
    envs = []
    inputs = []
    sources = []
    diagnostics = []
    statuses = []
    tidy_diagnostics = []
    drivers = []
    cuda_indices = [i for i in range(len(actions)) if actions[i].mnemonic == "CudaCompile"]
    cuda_keep = [cuda_indices[j] for j in nvcc_syntax_select([actions[i].argv or [] for i in cuda_indices])]
    for i in range(len(actions)):
        action = actions[i]
        argv = action.argv
        if argv == None:
            fail("clang_syntax: Bazel does not expose the command line of the %s action of %s" % (action.mnemonic, target.label))
        if action.mnemonic == "CudaCompile":
            # nvcc compiles each source several times (pic/rdc variants); lint one.
            if i not in cuda_keep:
                continue
            src = nvcc_syntax_source(list(argv))
            tidy_argv = cuda_tidy_argv(tidy.path, list(argv), src, tidy_config)
            tidy_log = ctx.actions.declare_file("dx_clang_tidy/%s_%d.log" % (target.label.name, i))
            if tidy_argv.error != "":
                _failing_tidy(ctx, tidy_log, src, tidy_argv.error)
                tidy_diagnostics.append(tidy_log)
                continue
            # run_shell: The shell redirects output or runs the bash runner script.
            ctx.actions.run_shell(
                mnemonic = "ClangTidy",
                progress_message = "clang-tidy %{label}: " + src,
                command = 'bash "$@"',
                arguments = [runner.path, tidy_log.path] + tidy_argv.argv,
                inputs = depset([tidy, runner], transitive = [action.inputs, headers, tidy_closure]),
                outputs = [tidy_log],
                env = action.env,
                resource_set = dx_heavy_resources,
            )
            tidy_diagnostics.append(tidy_log)
            continue
        src = clang_syntax_source(list(argv))
        rewritten = clang_syntax_argv(list(argv), src, identity)
        if rewritten.error != "":
            fail("clang_syntax: " + str(target.label) + ": " + rewritten.error)
        stem = "%s_%d" % (target.label.name, i)
        log = ctx.actions.declare_file("dx_clang_syntax/%s.log" % stem)
        status = ctx.actions.declare_file("dx_clang_syntax/%s.exit" % stem)
        all_inputs = depset(transitive = [action.inputs, headers])
        driver = ctx.actions.declare_file("dx_clang_syntax/%s.driver" % stem)
        ctx.actions.write(output = driver, content = identity.driver)
        tidy_argv = clang_tidy_argv(tidy.path, list(argv), src, identity, tidy_config)
        tidy_log = ctx.actions.declare_file("dx_clang_tidy/%s.log" % stem)
        requirements = {}
        if action.mnemonic == "ObjcCompile":
            requirements = dict(xcode_config.execution_info())
        capture = 'log="$1"; status="$2"; shift 2; "$@" 2> "$log"; echo "$?" > "$status"'
        if identity.driver == "msvc":
            capture = 'log="$1"; status="$2"; shift 2; "$@" > "$log" 2>&1; echo "$?" > "$status"'
        syntax_env = dict(action.env)
        if identity.driver == "msvc":
            syntax_env.update(_MSYS_NO_CONVERSION)
        # run_shell: The shell redirects output or runs the bash runner script.
        ctx.actions.run_shell(
            mnemonic = "ClangSyntax",
            progress_message = "Syntax check %{label}: " + src,
            command = capture,
            arguments = [log.path, status.path] + rewritten.argv,
            inputs = all_inputs,
            outputs = [log, status],
            env = syntax_env,
            execution_requirements = requirements,
            resource_set = dx_heavy_resources,
        )
        if tidy_argv.error != "":
            _failing_tidy(ctx, tidy_log, src, tidy_argv.error)
        else:
            # run_shell: The shell redirects output or runs the bash runner script.
            ctx.actions.run_shell(
                mnemonic = "ClangTidy",
                progress_message = "clang-tidy %{label}: " + src,
                command = 'bash "$@"',
                arguments = [runner.path, tidy_log.path] + tidy_argv.argv,
                inputs = depset([tidy, runner], transitive = [all_inputs, tidy_closure]),
                outputs = [tidy_log],
                env = action.env,
                execution_requirements = requirements,
                resource_set = dx_heavy_resources,
            )
        argvs.append(list(argv))
        envs.append(dict(action.env))
        inputs.append(all_inputs)
        tidy_diagnostics.append(tidy_log)
        sources.append(src)
        diagnostics.append(log)
        statuses.append(status)
        drivers.append(driver)
    return [
        ClangSyntaxInfo(argv = argvs, envs = envs, inputs = inputs, sources = sources, diagnostics = diagnostics, drivers = drivers, statuses = statuses, tidy_diagnostics = tidy_diagnostics),
        OutputGroupInfo(clang_syntax = depset(diagnostics + statuses + drivers), clang_tidy = depset(tidy_diagnostics)),
    ]

clang_syntax_aspect = aspect(
    implementation = _clang_syntax_impl,
    doc = "Replays the compile actions of a C/C++/Objective-C target with -fsyntax-only.",
    attr_aspects = ["upstream"],
    toolchains = [config_common.toolchain_type(CC_TOOLCHAIN_TYPE, mandatory = False)],
    attrs = apple_support.action_required_attrs() | {
        "_policy": attr.label(
            default = "//config:workspace",
            providers = [QualityPolicyInfo],
        ),
        "_clang_tidy": attr.label(default = "@llvm//tools:clang-tidy", allow_single_file = True, cfg = "exec"),
        "_clang_tidy_default": attr.label(default = "//quality:default.clang-tidy", allow_single_file = True),
        "_clang_tidy_run": attr.label(default = "//quality:clang_tidy_run.sh", allow_single_file = True, cfg = "exec"),
    },
)
