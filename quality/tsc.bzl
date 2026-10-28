"""Type-checks a typescript_project with its own tsc and records the diagnostics."""

load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load("@aspect_rules_ts//ts/private:options.bzl", "OptionsInfo")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

TscInfo = provider(
    doc = "Output of one non-failing tsc run over a TypeScript project.",
    fields = {
        "status": "File: tsc exit code.",
        "stdout": "File: tsc diagnostics (`--pretty false`), cwd-relative to the bin directory.",
    },
)

_RULES_TS = "aspect_rules_ts 3.10.1"

# Flags rules_ts adds under --@aspect_rules_ts//ts:verbose that would corrupt the diagnostics.
_VERBOSE_FLAGS = ["--listFiles", "--listEmittedFiles", "--traceResolution", "--diagnostics", "--extendedDiagnostics"]

def _bin_relative(ctx, file):
    prefix = ctx.bin_dir.path + "/"
    if not file.path.startswith(prefix):
        fail("tsc: " + file.path + " is not under " + prefix)
    return file.path[len(prefix):]

def _first(value):
    return value[0] if type(value) == "list" else value

def _is_ts_project(attrs):
    return hasattr(attrs, "tsc") and hasattr(attrs, "_options") and hasattr(attrs, "tsconfig")

def _tsc_impl(target, ctx):
    attrs = ctx.rule.attr
    if not _is_ts_project(attrs):
        if not hasattr(attrs, "upstream"):
            return []
        upstream = _first(attrs.upstream)
        if TscInfo not in upstream:
            return []
        return [upstream[TscInfo]]
    if OutputGroupInfo not in target or not hasattr(target[OutputGroupInfo], "_action_inputs"):
        fail("tsc: " + str(target.label) + " exposes no _action_inputs output group; the dx tsc aspect needs " + _RULES_TS)
    inputs = target[OutputGroupInfo]._action_inputs
    tsconfig_short = ctx.rule.file.tsconfig.short_path
    tsconfig = None
    for f in inputs.to_list():
        if f.short_path == tsconfig_short and not f.is_source:
            tsconfig = f
            break
    if tsconfig == None:
        fail("tsc: " + str(target.label) + " has no bin copy of " + tsconfig_short + " in _action_inputs; the dx tsc aspect needs " + _RULES_TS)

    stdout = ctx.actions.declare_file("dx_tsc/%s.stdout" % target.label.name)
    status = ctx.actions.declare_file("dx_tsc/%s.exit" % target.label.name)
    args = ctx.actions.args()
    args.add_all([a for a in attrs._options[OptionsInfo].args if a not in _VERBOSE_FLAGS])
    args.add_all(attrs.args)
    args.add("--project", _bin_relative(ctx, tsconfig))
    args.add("--noEmit")
    args.add("--pretty", "false")
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "TscDiagnostics",
        progress_message = "tsc diagnostics %{label}",
        executable = attrs.tsc[DefaultInfo].files_to_run,
        inputs = inputs,
        outputs = [stdout, status],
        arguments = [args],
        env = {
            "JS_BINARY__STDOUT_OUTPUT_FILE": _bin_relative(ctx, stdout),
            "JS_BINARY__EXIT_CODE_OUTPUT_FILE": _bin_relative(ctx, status),
        },
        use_default_shell_env = True,
        resource_set = dx_quality_heavy_resources,
    )
    return [TscInfo(stdout = stdout, status = status)]

tsc_aspect = aspect(
    implementation = _tsc_impl,
    doc = "Runs the project's own tsc with --noEmit over a typescript_project and records stdout and the exit code without failing the build.",
    attr_aspects = ["upstream"],
)
