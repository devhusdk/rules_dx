"""Type-checks a typescript_library with its own tsc and records the diagnostics."""

load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("@aspect_rules_ts//ts/private:options.bzl", "OptionsInfo")
load("//core:resources.bzl", "dx_heavy_resources")

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

_FRAMEWORKS = ["astro", "svelte", "vue", "mdx"]

_JS_EXTENSIONS = ("js", "jsx", "mjs", "cjs")

def _launcher(ctx, framework):
    return {
        "astro": ctx.executable._astro_tsc,
        "mdx": ctx.executable._mdx_tsc,
        "svelte": ctx.executable._svelte_tsc,
        "vue": ctx.executable._vue_tsc,
    }.get(framework, ctx.executable._tsc)

def _project_tsconfig(target):
    """The deepest tsconfig.json among the target's sources, or None."""
    found = None
    for f in target[JsInfo].sources.to_list():
        if f.basename == "tsconfig.json" and (found == None or len(f.path) > len(found.path)):
            found = f
    return found

def _tsc_js_run(ctx, target, name, framework, files, base, up):
    """Runs one checker over files with a generated tsconfig that extends base."""
    tsconfig = ctx.actions.declare_file("dx_tsc/%s.tsconfig.json" % name)
    stdout = ctx.actions.declare_file("dx_tsc/%s.stdout" % name)
    status = ctx.actions.declare_file("dx_tsc/%s.exit" % name)
    defaults = ctx.actions.declare_file("dx_tsc/%s.defaults.json" % name)
    ctx.actions.write(defaults, json.encode({"compilerOptions": {
        "module": "ESNext",
        "moduleResolution": "Bundler",
        "skipLibCheck": True,
        "target": "ES2022",
    }}))
    options = {
        "allowJs": True,
        "checkJs": True,
        "noEmit": True,
    }
    extra = []
    if framework == "mdx":
        options["jsx"] = "preserve"

        # MDX elements are HTML tags with no React types, so declare them to keep strict mode quiet.
        jsx = ctx.actions.declare_file("dx_tsc/%s.jsx.d.ts" % name)
        ctx.actions.write(jsx, "declare namespace JSX { interface IntrinsicElements { [name: string]: any } }\n")
        extra.append(jsx)
    ctx.actions.write(tsconfig, json.encode({
        "compilerOptions": options,
        "extends": ["./%s.defaults.json" % name, up + base.path],
        "files": [up + f.path for f in files + extra],
        "include": [],
    }))
    args = ctx.actions.args()
    args.add("--project", _bin_relative(ctx, tsconfig))
    args.add("--pretty", "false")
    npm = [target[JsInfo].npm_sources] if framework != None and framework != "mdx" else []
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "TscDiagnostics",
        progress_message = "tsc diagnostics (js) %{label}",
        executable = _launcher(ctx, framework),
        inputs = depset([tsconfig, defaults, base] + extra, transitive = [target[JsInfo].sources, target[JsInfo].transitive_sources] + npm),
        outputs = [stdout, status],
        arguments = [args],
        env = {
            "JS_BINARY__STDOUT_OUTPUT_FILE": _bin_relative(ctx, stdout),
            "JS_BINARY__EXIT_CODE_OUTPUT_FILE": _bin_relative(ctx, status),
        },
        resource_set = dx_heavy_resources,
    )
    return stdout, status

def _tsc_js_impl(target, ctx):
    """Type-checks a js_library's sources with one checker per framework, extending the project tsconfig."""
    sources = [f for f in target[JsInfo].sources.to_list() if f.extension in _JS_EXTENSIONS + tuple(_FRAMEWORKS)]
    if len(sources) == 0:
        return []
    base = _project_tsconfig(target) or ctx.file._tier_tsconfig

    # tsc runs with the bin directory as cwd; the tsconfig lives in <bin>/<pkg>/dx_tsc.
    depth = len(ctx.bin_dir.path.split("/")) + len(target.label.package.split("/")) + 1
    if target.label.package == "":
        depth = len(ctx.bin_dir.path.split("/")) + 1
    up = "../" * depth

    declarations = [f for f in target[JsInfo].sources.to_list() if f.path.endswith(".d.ts")]
    present = [fw for fw in _FRAMEWORKS if any([f.extension == fw for f in sources])]
    plain = [f for f in sources if f.extension in _JS_EXTENSIONS] + declarations
    outputs = []
    if len(present) == 0:
        outputs.append(_tsc_js_run(ctx, target, target.label.name, None, plain, base, up))
    for i, fw in enumerate(present):
        files = [f for f in sources if f.extension == fw] + (plain if i == 0 else [])
        name = target.label.name if len(present) == 1 else "%s.%s" % (target.label.name, fw)
        outputs.append(_tsc_js_run(ctx, target, name, fw, files, base, up))
    if len(outputs) == 1:
        stdout, status = outputs[0]
        return [TscInfo(stdout = stdout, status = status, inputs = depset(), tsconfig = None)]

    stdout = ctx.actions.declare_file("dx_tsc/%s.stdout" % target.label.name)
    status = ctx.actions.declare_file("dx_tsc/%s.exit" % target.label.name)
    args = ctx.actions.args()
    args.add(stdout)
    args.add(status)
    for out, code in outputs:
        args.add(out)
        args.add(code)

    # run_shell: concatenates the checkers' diagnostics and keeps the highest exit code.
    ctx.actions.run_shell(
        inputs = [f for pair in outputs for f in pair],
        outputs = [stdout, status],
        command = "out=$1; st=$2; shift 2; : > \"$out\"; worst=0; " +
                  "while [ $# -gt 0 ]; do cat \"$1\" >> \"$out\"; c=$(cat \"$2\"); " +
                  "if [ \"$c\" -gt \"$worst\" ]; then worst=$c; fi; shift 2; done; printf %s \"$worst\" > \"$st\"",
        arguments = [args],
        mnemonic = "TscMerge",
        progress_message = "merging tsc diagnostics %{label}",
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
    doc = "Runs the project's own tsc with --noEmit over a typescript_library and records stdout and the exit code without failing the build.",
    attr_aspects = ["upstream"],
    attrs = {
        "_astro_tsc": attr.label(
            default = "//javascript/private/quality/bin:astro_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_mdx_tsc": attr.label(
            default = "//javascript/private/quality/bin:mdx_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_svelte_tsc": attr.label(
            default = "//javascript/private/quality/bin:svelte_tsc",
            executable = True,
            cfg = "exec",
        ),
        "_tier_tsconfig": attr.label(
            default = "//quality/tiers/tsconfig:standard.json",
            allow_single_file = True,
        ),
        "_tsc": attr.label(
            default = "@npm_typescript//:tsc",
            executable = True,
            cfg = "exec",
        ),
        "_vue_tsc": attr.label(
            default = "//javascript/private/quality/bin:vue_tsc",
            executable = True,
            cfg = "exec",
        ),
    },
)
