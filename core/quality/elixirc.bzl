"""Replays the elixirc compilation of a target and records the compiler warnings."""

load("@rules_elixir//private:elixir_toolchain.bzl", "elixir_dirs", "erlang_dirs")
load("@rules_erlang//:erlang_app_info.bzl", "flat_deps")
load("@rules_erlang//:util.bzl", "path_join")
load("@rules_erlang//private:util.bzl", "erl_libs_contents")
load("@rules_erlang//tools:erlang_toolchain.bzl", "TREE_ERLANG_HOME")
load("//core:resources.bzl", "dx_medium_resources")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "applet_file_name", "coreutils")

visibility(["//core/...", "//erlang/..."])

_TOOLCHAIN = Label("@rules_elixir//:toolchain_type")

ElixircInfo = provider(
    doc = "elixirc output from compiling the sources of one elixir_bytecode target.",
    fields = {
        "logs": "list of File: elixirc output per elixir_bytecode target.",
        "names": "list of string: the target name per entry.",
        "statuses": "list of File: elixirc exit code per entry.",
    },
)

def _targets(attrs, name):
    value = getattr(attrs, name, None)
    if value == None:
        return []
    return value if type(value) == "list" else [value]

_BIN = "/bin"

def _absolute(path):
    if path.startswith("/"):
        return path
    return "__BAZEL_EXECUTION_ROOT__/" + path

def elixirc_command(env_program, elixir_home, erlang_root, coreutils_path, erl_libs_path, env, out, elixirc_opts, srcs):
    """Returns the argv of one elixirc replay, with placeholders for the execution root."""
    path = ":".join([_absolute(elixir_home) + _BIN, _absolute(erlang_root) + _BIN, _absolute(coreutils_path)])
    command = [_absolute(env_program), "PATH=" + path, "RULES_ERLANG_HOME=" + _absolute(erlang_root)]
    if erl_libs_path:
        command.append("ERL_LIBS=" + _absolute(erl_libs_path))
    command += ["%s=%s" % (k, v) for k, v in env.items()]
    return command + [_absolute(elixir_home) + _BIN + "/elixirc", "-o", out] + elixirc_opts + srcs

def _elixirc_impl(target, ctx):
    attrs = ctx.rule.attr
    if ctx.rule.kind != "elixir_bytecode":
        infos = [t[ElixircInfo] for name in ["upstream", "beam", "beam_files_dir"] for t in _targets(attrs, name) if ElixircInfo in t]
        if len(infos) == 0:
            return []
        return [ElixircInfo(
            logs = [log for info in infos for log in info.logs],
            names = [name for info in infos for name in info.names],
            statuses = [status for info in infos for status in info.statuses],
        )]

    toolchain = ctx.toolchains[_TOOLCHAIN]
    if toolchain == None or ctx.target_platform_has_constraint(ctx.attr._windows_constraint[platform_common.ConstraintValueInfo]):
        return []

    elixir_ctx = struct(toolchains = {"//:toolchain_type": toolchain}, runfiles = ctx.runfiles)
    name = target.label.name
    deps_dir = "dx_elixirc/%s_deps" % name
    erl_libs_files = erl_libs_contents(
        ctx,
        target_info = None,
        headers = True,
        dir = deps_dir,
        deps = flat_deps(attrs.deps),
        ez_deps = ctx.rule.files.ez_deps,
        expand_ezs = True,
    )
    erl_libs_path = ""
    if len(erl_libs_files) > 0:
        (output_dir, _, _) = erl_libs_files[0].path.partition(deps_dir)
        erl_libs_path = path_join(output_dir.rstrip("/"), deps_dir)

    (erlang_home, _, erlang_runfiles) = erlang_dirs(elixir_ctx)
    (elixir_home, elixir_runfiles) = elixir_dirs(elixir_ctx)
    if erlang_home != TREE_ERLANG_HOME:
        fail("%s: the elixirc replay needs an OTP tree toolchain" % target.label)
    if attrs.setup != "":
        fail("%s: the elixirc replay does not run the setup attribute" % target.label)
    erlang_root = elixir_ctx.toolchains["//:toolchain_type"].otpinfo.version_file.dirname
    opts = [o for o in attrs.elixirc_opts if o != "--warnings-as-errors"]

    applets = coreutils(ctx)
    log = ctx.actions.declare_file("dx_elixirc/%s/%s.log" % (name, name))
    status = ctx.actions.declare_file("dx_elixirc/%s/%s.exit" % (name, name))
    out_tree = ctx.actions.declare_directory("dx_elixirc/%s/out" % name)

    command = elixirc_command(
        env_program = applets.path + "/" + applet_file_name("env", applets.binary.basename),
        elixir_home = elixir_home,
        erlang_root = erlang_root,
        coreutils_path = applets.path,
        erl_libs_path = erl_libs_path,
        env = attrs.env,
        out = out_tree.path,
        elixirc_opts = opts,
        srcs = [f.path for f in ctx.rule.files.srcs],
    )
    ctx.actions.run(
        mnemonic = "ElixircWarnings",
        progress_message = "elixirc diagnostics %{label}",
        executable = ctx.executable._capture,
        arguments = ["--stdout", log.path, "--stderr", log.path, "--status", status.path, "--expand-bazel", "--"] + command,
        inputs = depset(
            direct = ctx.rule.files.srcs + ctx.rule.files.data + erl_libs_files,
            transitive = [erlang_runfiles.files, elixir_runfiles.files, applets.files],
        ),
        outputs = [log, status, out_tree],
        resource_set = dx_medium_resources,
        toolchain = _TOOLCHAIN,
    )
    return [ElixircInfo(logs = [log], names = [name], statuses = [status])]

elixirc_aspect = aspect(
    implementation = _elixirc_impl,
    doc = "Replays the elixirc compilation of a target and records the compiler warnings.",
    attr_aspects = ["upstream", "beam", "beam_files_dir"],
    attrs = {
        "_capture": attr.label(
            default = "//core/quality/capture",
            executable = True,
            cfg = "exec",
            doc = "Runs one program with its streams and exit code written to files.",
        ),
        "_windows_constraint": attr.label(default = "@platforms//os:windows"),
    },
    toolchains = [config_common.toolchain_type(_TOOLCHAIN, mandatory = False), COREUTILS_TOOLCHAIN_TYPE],
)
