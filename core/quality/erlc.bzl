"""Replays the erlc compilation of a target and records the compiler warnings."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("@rules_erlang//:util.bzl", "path_join")
load("@rules_erlang//private:util.bzl", "erl_libs_contents")
load("//core:resources.bzl", "dx_medium_resources")

_TOOLCHAIN = "@rules_erlang//tools:toolchain_type"

ErlcInfo = provider(
    doc = "erlc output from compiling the sources of one erlang_bytecode target.",
    fields = {
        "logs": "list of File: erlc output per erlang_bytecode target.",
        "names": "list of string: the target name per entry.",
        "statuses": "list of File: erlc exit code per entry.",
    },
)

def _targets(attrs, name):
    value = getattr(attrs, name, None)
    if value == None:
        return []
    return value if type(value) == "list" else [value]

def _erlc_impl(target, ctx):
    attrs = ctx.rule.attr
    if ctx.rule.kind != "erlang_bytecode":
        infos = [t[ErlcInfo] for name in ["upstream", "beam"] for t in _targets(attrs, name) if ErlcInfo in t]
        if len(infos) == 0:
            return []
        return [ErlcInfo(
            logs = [log for info in infos for log in info.logs],
            names = [name for info in infos for name in info.names],
            statuses = [status for info in infos for status in info.statuses],
        )]

    toolchain = ctx.toolchains[_TOOLCHAIN]
    if toolchain == None or not hasattr(attrs, "dest"):
        return []

    # rules_erlang reads its toolchain from ctx.toolchains under its own label.
    erlang_ctx = struct(toolchains = {"//tools:toolchain_type": toolchain}, runfiles = ctx.runfiles)
    name = target.label.name
    deps_dir = "dx_erlc/%s_deps" % name
    app = None
    if attrs.app_name != "":
        app = ErlangAppInfo(app_name = attrs.app_name, include = ctx.rule.files.hdrs)
    erl_libs_files = erl_libs_contents(
        ctx,
        target_info = app,
        headers = True,
        dir = deps_dir,
        deps = attrs.deps,
        ez_deps = ctx.rule.files.ez_deps,
    )
    erl_libs_path = ""
    if len(erl_libs_files) > 0:
        (output_dir, _, _) = erl_libs_files[0].path.partition(deps_dir)
        erl_libs_path = path_join(output_dir.rstrip("/"), deps_dir)

    include_args = []
    package_dir = path_join(ctx.label.workspace_root, ctx.label.package)
    if package_dir != "":
        include_args.extend(["-I", package_dir])
    if erl_libs_path != "":
        include_args.extend(["-I", erl_libs_path])
    for hdr_dir in {f.dirname: True for f in ctx.rule.files.hdrs}:
        include_args.extend(["-I", hdr_dir])
    pa_args = []
    for beam_dir in {f.dirname: True for f in ctx.rule.files.beam}:
        pa_args.extend(["-pa", beam_dir])

    opts = ["--opt=%s" % o for o in attrs.erlc_opts if o not in ["+warnings_as_errors", "-Werror"]]
    runfiles = ctx.runfiles()
    compile_first = []
    compile_first_target = attrs.compile_first
    if type(compile_first_target) == "list":
        compile_first_target = compile_first_target[0] if compile_first_target else None
    if compile_first_target != None:
        compile_first = ["--compile-first=" + compile_first_target[DefaultInfo].files_to_run.executable.path]
        runfiles = runfiles.merge(compile_first_target[DefaultInfo].default_runfiles)

    log = ctx.actions.declare_file("dx_erlc/%s/%s.log" % (name, name))
    status = ctx.actions.declare_file("dx_erlc/%s/%s.exit" % (name, name))
    out_tree = ctx.actions.declare_directory("dx_erlc/%s/out" % name)

    escript = ctx.attr._escript[DefaultInfo].files_to_run
    args = ctx.actions.args()
    args.add(ctx.file._replay)
    args.add_all(compile_first)
    args.add(log, format = "--log=%s")
    args.add(status, format = "--status=%s")
    args.add(out_tree.path, format = "--out=%s")
    if erl_libs_path != "":
        args.add(erl_libs_path, format = "--erl-libs=%s")
    args.add_all(opts)
    args.add_all(include_args + pa_args, format_each = "--erlc-arg=%s")
    args.add("--")
    args.add_all(ctx.rule.files.srcs)
    ctx.actions.run(
        mnemonic = "ErlcWarnings",
        progress_message = "erlc diagnostics %{label}",
        executable = escript,
        arguments = [args],
        tools = [escript],
        inputs = depset(
            direct = [ctx.file._replay] + ctx.rule.files.hdrs + ctx.rule.files.srcs + ctx.rule.files.beam + erl_libs_files,
            transitive = [runfiles.files],
        ),
        outputs = [log, status, out_tree],
        resource_set = dx_medium_resources,
        toolchain = _TOOLCHAIN,
    )
    return [ErlcInfo(logs = [log], names = [name], statuses = [status])]

erlc_aspect = aspect(
    implementation = _erlc_impl,
    doc = "Replays the erlc compilation of a target and records the compiler warnings.",
    attr_aspects = ["upstream", "beam"],
    attrs = {
        "_escript": attr.label(default = "//erlang/private/tools:escript_bin", cfg = "exec", executable = True),
        "_replay": attr.label(default = "//core/quality:erlc_replay.escript", allow_single_file = True),
    },
    toolchains = [config_common.toolchain_type(_TOOLCHAIN, mandatory = False)],
)
