"""Type-checks a typescript_project with its own tsc and records the diagnostics."""

load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("@aspect_rules_ts//ts/private:options.bzl", "OptionsInfo")
load("//libs/starlark:resources.bzl", "dx_heavy_resources")

TscInfo = provider(
    doc = "Output of one non-failing tsc run over a TypeScript project.",
    fields = {
        "inputs": "depset of File: what the ts_project's tsc action reads, empty for js_library.",
        "status": "File: tsc exit code.",
        "stdout": "File: tsc diagnostics (`--pretty false`), cwd-relative to the bin directory.",
        "tsconfig": "File: the bin copy of the ts_project's tsconfig, None for js_library.",
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

def _js_launcher(ctx, has_astro, has_svelte, has_vue, has_mdx):
    for present, launcher in [
        (has_astro, ctx.executable._astro_tsc),
        (has_svelte, ctx.executable._svelte_tsc),
        (has_vue, ctx.executable._vue_tsc),
        (has_mdx, ctx.executable._mdx_tsc),
    ]:
        if present:
            return launcher
    return ctx.executable._tsc

def _tsc_js_impl(target, ctx):
    """Type-checks a js_library's JavaScript sources with checkJs through a generated tsconfig."""
    sources = [f for f in target[JsInfo].sources.to_list() if f.extension in ("js", "jsx", "mjs", "cjs", "astro", "mdx", "svelte", "vue")]
    has_mdx = any([f.extension == "mdx" for f in sources])
    has_vue = any([f.extension == "vue" for f in sources])
    has_svelte = any([f.extension == "svelte" for f in sources])
    has_astro = any([f.extension == "astro" for f in sources])
    if len(sources) == 0:
        return []
    tsconfig = ctx.actions.declare_file("dx_tsc/%s.tsconfig.json" % target.label.name)
    stdout = ctx.actions.declare_file("dx_tsc/%s.stdout" % target.label.name)
    status = ctx.actions.declare_file("dx_tsc/%s.exit" % target.label.name)

    # tsc runs with the bin directory as cwd; the tsconfig lives in <bin>/<pkg>/dx_tsc.
    depth = len(ctx.bin_dir.path.split("/")) + len(target.label.package.split("/")) + 1
    if target.label.package == "":
        depth = len(ctx.bin_dir.path.split("/")) + 1
    up = "../" * depth
    options = {
        "allowJs": True,
        "checkJs": True,
        "module": "ESNext",
        "moduleResolution": "Bundler",
        "noEmit": True,
        "skipLibCheck": True,
        "target": "ES2022",
    }
    if has_mdx:
        options["jsx"] = "preserve"
    ctx.actions.write(tsconfig, json.encode({
        "compilerOptions": options,
        "files": [up + f.path for f in sources],
    }))
    args = ctx.actions.args()
    args.add("--project", _bin_relative(ctx, tsconfig))
    args.add("--pretty", "false")
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "TscDiagnostics",
        progress_message = "tsc diagnostics (js) %{label}",
        executable = _js_launcher(ctx, has_astro, has_svelte, has_vue, has_mdx),
        inputs = depset([tsconfig], transitive = [target[JsInfo].sources, target[JsInfo].transitive_sources] + ([target[JsInfo].npm_sources] if has_vue or has_svelte or has_astro else [])),
        outputs = [stdout, status],
        arguments = [args],
        env = {
            "JS_BINARY__STDOUT_OUTPUT_FILE": _bin_relative(ctx, stdout),
            "JS_BINARY__EXIT_CODE_OUTPUT_FILE": _bin_relative(ctx, status),
        },
        resource_set = dx_heavy_resources,
    )
    return [TscInfo(stdout = stdout, status = status, inputs = depset(), tsconfig = None)]

def _tsc_impl(target, ctx):
    attrs = ctx.rule.attr
    if ctx.rule.kind == "js_library" and JsInfo in target:
        return _tsc_js_impl(target, ctx)
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
        resource_set = dx_heavy_resources,
    )
    return [TscInfo(stdout = stdout, status = status, inputs = inputs, tsconfig = tsconfig)]

tsc_aspect = aspect(
    implementation = _tsc_impl,
    doc = "Runs the project's own tsc with --noEmit over a typescript_project and records stdout and the exit code without failing the build.",
    attr_aspects = ["upstream"],
    attrs = {
        "_astro_tsc": attr.label(
            default = "//quality/tools/javascript/bin:astro_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_mdx_tsc": attr.label(
            default = "//quality/tools/javascript/bin:mdx_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_svelte_tsc": attr.label(
            default = "//quality/tools/javascript/bin:svelte_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_tsc": attr.label(
            default = "@npm_typescript//:tsc",
            executable = True,
            cfg = "exec",
        ),
        "_vue_tsc": attr.label(
            default = "//quality/tools/javascript/bin:vue_tsc",
            executable = True,
            cfg = "exec",
        ),
    },
)
