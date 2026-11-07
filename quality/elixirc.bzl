"""Replays the elixirc compilation of a target and records the compiler warnings."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("@rules_elixir//private:elixir_toolchain.bzl", "elixir_dirs", "erlang_dirs", "maybe_install_erlang")
load("@rules_erlang//:erlang_app_info.bzl", "flat_deps")
load("@rules_erlang//:util.bzl", "path_join")
load("@rules_erlang//private:util.bzl", "erl_libs_contents")
load("//libs/starlark:resources.bzl", "dx_medium_resources")

_TOOLCHAIN = Label("//elixir/toolchain/type:toolchain_type")

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
    if toolchain == None:
        return []

    # rules_elixir reads its toolchain from ctx.toolchains under its own label.
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
    opts = [shell.quote(o) for o in attrs.elixirc_opts if o != "--warnings-as-errors"]
    env = "\n".join(["export %s=%s" % (k, v) for k, v in attrs.env.items()])

    log = ctx.actions.declare_file("dx_elixirc/%s/%s.log" % (name, name))
    status = ctx.actions.declare_file("dx_elixirc/%s/%s.exit" % (name, name))
    out_tree = ctx.actions.declare_directory("dx_elixirc/%s/out" % name)

    script = """log="$1"; status="$2"; out="$3"; shift 3
{maybe_install_erlang}
case "{elixir_home}" in
    /*) ABS_ELIXIR_HOME="{elixir_home}" ;;
    *) ABS_ELIXIR_HOME="$PWD/{elixir_home}" ;;
esac
export PATH="$ABS_ELIXIR_HOME"/bin:"{erlang_home}"/bin:"$PATH"
if [ -n "{erl_libs_path}" ]; then
    export ERL_LIBS="{erl_libs_path}"
fi
{env}
{setup}
"$ABS_ELIXIR_HOME"/bin/elixirc -o "$out" {elixirc_opts} "$@" > "$log" 2>&1
echo "$?" > "$status"
""".format(
        maybe_install_erlang = maybe_install_erlang(elixir_ctx),
        elixir_home = elixir_home,
        erlang_home = erlang_home,
        erl_libs_path = erl_libs_path,
        env = env,
        setup = attrs.setup,
        elixirc_opts = " ".join(opts),
    )
    srcs = ctx.actions.args()
    srcs.add_all(ctx.rule.files.srcs)
    ctx.actions.run_shell(
        mnemonic = "ElixircWarnings",
        progress_message = "elixirc diagnostics %{label}",
        command = script,
        arguments = [log.path, status.path, out_tree.path, srcs],
        inputs = depset(
            direct = ctx.rule.files.srcs + ctx.rule.files.data + erl_libs_files,
            transitive = [erlang_runfiles.files, elixir_runfiles.files],
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
    toolchains = [config_common.toolchain_type(_TOOLCHAIN, mandatory = False)],
)
