"""Docs aspect: puts docs sites, shards and API trees of a target in output groups."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("@rules_julia//julia:julia_info.bzl", _JuliaInfo = "JuliaInfo")
load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo")
load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@rules_ruby//ruby/private:providers.bzl", _RubyFilesInfo = "RubyFilesInfo")
load("@rules_dart//dart:providers.bzl", _DartInfo = "DartInfo")
load("@rules_d//d/private:providers.bzl", _DInfo = "DInfo")
load("@rules_go//go:def.bzl", "GoInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")
load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load("//ocaml/rules:providers.bzl", _OCamlCmtInfo = "OCamlCmtInfo")
load("//libs/starlark:resources.bzl", "dx_heavy_resources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")
load("//tools/docs/components:shard.bzl", "COMPONENT_LANGUAGES", "component_config", "component_shard_action")
load("//tools/docs/dart:defs.bzl", "dart_shard_action")
load("//tools/docs/d:defs.bzl", "D_TOOLCHAIN", "d_shard_action")
load("//tools/docs/dotnet:shard.bzl", "DotnetDocsInfo", "dotnet_docs_collect_aspect", "dotnet_docs_problem", "dotnet_shard_action")
load("//tools/docs/go:defs.bzl", "go_shard_action")
load("//tools/docs/julia:defs.bzl", "julia_shard_action")
load("//tools/docs/ocaml:defs.bzl", "ODOC_CONFIG", "ocaml_odoc_available", "ocaml_shard_action", "ocaml_typed_trees")
load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "JVM_LANGUAGES", "KOTLIN_TOOLCHAIN", "jvm_shard_action")
load("//tools/docs/powershell:defs.bzl", "powershell_shard_action")
load("//tools/docs/python:shard.bzl", "python_package_name_ok", "python_shard_action")
load("//tools/docs/typescript:shard.bzl", "TYPEDOC_JSON_SCHEMA", "typedoc_arguments", "typescript_shard_action")
load("//tools/docs/ruby:defs.bzl", "ruby_shard_action")
load("//tools/docs/rust:defs.bzl", "rust_doc_inputs_aspect", "rust_shard_action")
load("//tools/docs/site:render.bzl", "DocsApiInfo", "DocsSiteInfo")

_RUST_DOCS_TOOLCHAIN = str(Label("@rules_rust//rust:toolchain_type"))
_RUBY_TOOLCHAIN = "@rules_ruby//ruby:toolchain_type"
_DART_TOOLCHAIN = "@rules_dart//dart:toolchain_type"
_CC_DOCS_TOOLCHAIN = "@bazel_tools//tools/cpp:toolchain_type"

def _rust_docs_shard(target, ctx):
    if _rust_common.crate_info not in target or DocsShardInfo in target:
        return None
    crate = target[_rust_common.crate_info]
    if crate.type not in ("rlib", "lib") or crate.is_test:
        return None
    if not ctx.toolchains[_RUST_DOCS_TOOLCHAIN] or not ctx.toolchains[_CC_DOCS_TOOLCHAIN]:
        return None
    return rust_shard_action(ctx, target, "", stem = "_dx_docs/" + ctx.label.name)

def _go_docs_shard(target, ctx):
    if GoInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[GoInfo]
    if not info.importpath:
        return None
    return go_shard_action(ctx, info, "", "_dx_docs/" + ctx.label.name, ctx.executable._driver, ctx.executable._go_extract).shard

def _python_docs_shard(target, ctx):
    if _PyInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    if not python_package_name_ok(ctx.label.name):
        return None
    action = python_shard_action(ctx, target[_PyInfo], ctx.label.name, "google", "_dx_docs/" + ctx.label.name, ctx.executable._py_driver, ctx.executable._py_extract)
    return action.shard if action else None

def _jvm_docs_shard(target, ctx):
    if JavaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or getattr(ctx.rule.attr, "plugins", []):
        return None
    for language, spec in JVM_LANGUAGES.items():
        if all([f.extension == spec.extension for f in srcs]):
            if ctx.toolchains[JAVA_TOOLCHAIN] == None or (language == "kotlin" and ctx.toolchains[KOTLIN_TOOLCHAIN] == None):
                return None
            adapter = getattr(ctx.executable, "_" + language + "_extract")
            action = jvm_shard_action(ctx, language, target[JavaInfo], srcs, ctx.label.name, [], "_dx_docs/" + ctx.label.name, adapter, ctx.executable._jvm_convert)
            return action.shard
    return None

def _dart_docs_shard(target, ctx):
    if _DartInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    info = target[_DartInfo]
    root = info.lib_root + "/lib/" if info.lib_root else "lib/"
    if ctx.toolchains[_DART_TOOLCHAIN] == None or not [f for f in info.srcs if repo_relative(f.short_path).startswith(root) and f.extension == "dart"]:
        return None
    return dart_shard_action(ctx, info, info.package_name, "_dx_docs/" + ctx.label.name, ctx.executable._dart_extract).shard
def _d_docs_shard(target, ctx):
    if _DInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension in ("d", "di")]
    if not own or ctx.toolchains[D_TOOLCHAIN] == None:
        return None
    return d_shard_action(ctx, target[_DInfo], own, ctx.label.name, "", "_dx_docs/" + ctx.label.name, ctx.executable._d_extract).shard

def _dotnet_docs_shard(target, ctx):
    if DocsShardInfo in target or DotnetDocsInfo not in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[DotnetDocsInfo]
    if dotnet_docs_problem(info, info.language):
        return None
    driver = ctx.executable._csharp_driver if info.language == "csharp" else ctx.executable._fsharp_driver
    return dotnet_shard_action(ctx, info, info.assembly, "_dx_docs/" + ctx.label.name, driver, ctx.executable._dotnet_extract).shard

def _powershell_docs_shard(target, ctx):
    if _PwshInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    files = sorted(target[_PwshInfo].srcs.to_list(), key = lambda f: f.path)
    sources = [f for f in files if f.extension in ("ps1", "psm1")]
    manifests = [f for f in files if f.extension == "psd1"]
    if not sources or len(manifests) > 1:
        return None
    package = manifests[0].basename[:-len(".psd1")] if manifests else ctx.label.name
    pwsh = ctx.attr._pwsh[platform_common.ToolchainInfo]
    return powershell_shard_action(ctx, sources, manifests[0] if manifests else None, package, "", "_dx_docs/" + ctx.label.name, pwsh, ctx.file._pwsh_script, ctx.executable._pwsh_extract).shard

def _ruby_docs_shard(target, ctx):
    if _RubyFilesInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension == "rb"]
    if not own or ctx.toolchains[_RUBY_TOOLCHAIN] == None:
        return None
    yard = ctx.attr._ruby_yard[DefaultInfo].files.to_list()
    return ruby_shard_action(ctx, own, target[_RubyFilesInfo].transitive_srcs.to_list(), [], ctx.label.name, "_dx_docs/" + ctx.label.name, yard, ctx.file._ruby_driver, ctx.executable._ruby_extract).shard

def _julia_docs_shard(target, ctx):
    if _JuliaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[_JuliaInfo]
    if not [f for f in info.srcs.to_list() if f.extension == "jl"]:
        return None
    return julia_shard_action(ctx, info.srcs.to_list(), info.transitive_srcs.to_list(), ctx.label.name, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._julia_driver, ctx.executable._julia_extract).shard

def _ocaml_docs_shard(target, ctx):
    if _OCamlCmtInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    if not ocaml_odoc_available(ctx.attr._odoc):
        return None
    trees = ocaml_typed_trees(target[_OCamlCmtInfo])
    if not trees:
        return None
    return ocaml_shard_action(ctx, trees, ctx.label.name, ODOC_CONFIG, "_dx_docs/" + ctx.label.name, ctx.executable._odoc, ctx.executable._ocaml_extract).shard

_TS_ENTRY_EXTENSIONS = ["ts", "tsx", "mts", "cts"]
_TS_TEST_MARKERS = [".test.", ".spec.", "_test."]

def _typescript_entry_point(file):
    name = file.basename
    if file.extension not in _TS_ENTRY_EXTENSIONS or ".d." in name:
        return False
    return not any([marker in name for marker in _TS_TEST_MARKERS])

def _typescript_docs_shard(target, ctx):
    attrs = ctx.rule.attr
    if DocsShardInfo in target or not (hasattr(attrs, "tsc") and hasattr(attrs, "_options") and hasattr(attrs, "tsconfig")):
        return None
    if ctx.rule.kind.endswith("_test") or OutputGroupInfo not in target or not hasattr(target[OutputGroupInfo], "_action_inputs"):
        return None
    entry_points = sorted([repo_relative(f.short_path) for f in ctx.rule.files.srcs if _typescript_entry_point(f)])
    if not entry_points:
        return None
    package = ctx.label.name
    tsconfig = repo_relative(ctx.rule.file.tsconfig.short_path)
    stem = "_dx_docs/" + ctx.label.name
    typedoc_json = ctx.actions.declare_file(stem + ".typedoc.json")
    arguments = ctx.actions.args()
    arguments.add_all(typedoc_arguments(tsconfig, package, typedoc_json.path[len(ctx.bin_dir.path) + 1:], entry_points, [], ctx.label.package or "."))
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "DxTypedoc",
        progress_message = "TypeDoc JSON of %{label}",
        executable = ctx.attr._typedoc[DefaultInfo].files_to_run,
        inputs = target[OutputGroupInfo]._action_inputs,
        outputs = [typedoc_json],
        arguments = [arguments],
        resource_set = dx_heavy_resources,
    )
    config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + TYPEDOC_JSON_SCHEMA + ";options="
    return typescript_shard_action(ctx, typedoc_json, package, ctx.label.package, config, stem, ctx.executable._ts_extract)
_JS_ENTRY_EXTENSIONS = ["js", "mjs", "cjs", "jsx"]

def _javascript_docs_shard(target, ctx):
    if _JsInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or not all([f.extension in _JS_ENTRY_EXTENSIONS for f in srcs]):
        return None
    js = target[_JsInfo]
    package = ctx.label.package
    tsconfigs = [f for f in js.transitive_sources.to_list() if f.basename == "tsconfig.json" and f.dirname.endswith("/" + package if package else "")]
    if len(tsconfigs) != 1:
        return None
    entry_points = sorted([repo_relative(f.short_path) for f in srcs if not any([marker in f.basename for marker in _TS_TEST_MARKERS])])
    if not entry_points:
        return None
    tsconfig = repo_relative(tsconfigs[0].short_path)
    stem = "_dx_docs/" + ctx.label.name
    typedoc_json = ctx.actions.declare_file(stem + ".typedoc.json")
    arguments = ctx.actions.args()
    arguments.add_all(typedoc_arguments(tsconfig, ctx.label.name, typedoc_json.path[len(ctx.bin_dir.path) + 1:], entry_points, [], package or "."))
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "DxTypedoc",
        progress_message = "TypeDoc JSON of %{label}",
        executable = ctx.attr._typedoc[DefaultInfo].files_to_run,
        inputs = depset(transitive = [js.transitive_sources, js.npm_sources]),
        outputs = [typedoc_json],
        arguments = [arguments],
        resource_set = dx_heavy_resources,
    )
    config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + TYPEDOC_JSON_SCHEMA + ";options="
    return typescript_shard_action(ctx, typedoc_json, ctx.label.name, package, config, stem, ctx.executable._ts_extract)

def _component_docs_shard(target, ctx):
    if _JsInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    for language, spec in COMPONENT_LANGUAGES.items():
        if srcs and all([f.extension == spec.extension for f in srcs]):
            return _component_shard(target[_JsInfo], ctx, language, srcs)
    return None

def _component_shard(js, ctx, language, srcs):
    inputs = depset(transitive = [js.transitive_sources, js.npm_sources])
    package = ctx.label.package
    tsconfigs = [f for f in js.transitive_sources.to_list() if f.basename == "tsconfig.json" and f.dirname.endswith("/" + package if package else "")]
    if len(tsconfigs) != 1:
        return None
    stem = "_dx_docs/" + ctx.label.name
    meta = ctx.actions.declare_file(stem + ".component_meta.json")
    entry_points = [f.short_path[len(package) + 1:] if package else f.short_path for f in srcs]
    tsconfig = tsconfigs[0].basename
    args = ctx.actions.args()
    args.add("--bazel-bindir", ctx.bin_dir.path)
    args.add("--tsconfig", tsconfig)
    args.add("--out", stem + ".component_meta.json")
    for entry in entry_points:
        args.add("--entry", entry)
    tool = getattr(ctx.attr, "_" + language + "_meta")
    ctx.actions.run(
        executable = getattr(ctx.executable, "_" + language + "_meta"),
        arguments = [args],
        inputs = inputs,
        tools = [tool[DefaultInfo].files_to_run],
        outputs = [meta],
        mnemonic = COMPONENT_LANGUAGES[language].mnemonic,
        progress_message = "Reading component metadata of %{label}",
        env = {"JS_BINARY__CHDIR": package, "JS_BINARY__PATCH_NODE_FS": "1"},
        resource_set = _mem_2g,
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    component_shard_action(ctx, ctx.executable._component_extract, meta, shard, language, ctx.label.name, package, component_config(language, tsconfig, entry_points))
    return shard

def _mem_2g(_os, _inputs):
    return {"memory": 2048}

def _component_meta_attr(language):
    return attr.label(
        cfg = "exec",
        default = Label("//quality/tools/javascript/bin:" + language + "_component_meta"),
        executable = True,
    )

def _docs_site_groups(target, ctx):
    tree = target[DocsSiteInfo].tree
    manifest = ctx.actions.declare_file(ctx.label.name + ".dx_docs_site")
    ctx.actions.write(manifest, tree.path)
    return {"dx_docs_sites": [manifest], "dx_results": [tree]}

def _docs_api_groups(target):
    api = target[DocsApiInfo]
    return {"dx_results": [api.api, api.records, api.summary, api.tree]}

def _docs_shard_groups(target):
    return {"dx_results": [target[DocsShardInfo].shard]}

def _real_docs_aspect_impl(target, ctx):
    parts = []
    if DocsSiteInfo in target:
        parts.append(_docs_site_groups(target, ctx))
    if DocsApiInfo in target:
        parts.append(_docs_api_groups(target))
    if DocsShardInfo in target:
        parts.append(_docs_shard_groups(target))
    rust = _rust_docs_shard(target, ctx)
    if rust:
        parts.append({
            "dx_results": [rust.shard],
            "dx_docs_shards": [rust.shard],
            "rustdoc_json": rust.outputs["rustdoc_json"].to_list(),
            "srcs_manifest": rust.outputs["srcs_manifest"].to_list(),
        })
    go_shard = _go_docs_shard(target, ctx)
    if go_shard:
        parts.append({"dx_results": [go_shard], "dx_docs_shards": [go_shard]})
    py_shard = _python_docs_shard(target, ctx)
    if py_shard:
        parts.append({"dx_results": [py_shard], "dx_docs_shards": [py_shard]})
    jvm_shard = _jvm_docs_shard(target, ctx)
    if jvm_shard:
        parts.append({"dx_results": [jvm_shard], "dx_docs_shards": [jvm_shard]})
    dart_shard = _dart_docs_shard(target, ctx)
    if dart_shard:
        parts.append({"dx_results": [dart_shard], "dx_docs_shards": [dart_shard]})
    d_shard = _d_docs_shard(target, ctx)
    if d_shard:
        parts.append({"dx_results": [d_shard], "dx_docs_shards": [d_shard]})
    dotnet_shard = _dotnet_docs_shard(target, ctx)
    if dotnet_shard:
        parts.append({"dx_results": [dotnet_shard], "dx_docs_shards": [dotnet_shard]})
    pwsh_shard = _powershell_docs_shard(target, ctx)
    if pwsh_shard:
        parts.append({"dx_results": [pwsh_shard], "dx_docs_shards": [pwsh_shard]})
    ruby_shard = _ruby_docs_shard(target, ctx)
    if ruby_shard:
        parts.append({"dx_results": [ruby_shard], "dx_docs_shards": [ruby_shard]})
    julia_shard = _julia_docs_shard(target, ctx)
    if julia_shard:
        parts.append({"dx_results": [julia_shard], "dx_docs_shards": [julia_shard]})
    ocaml_shard = _ocaml_docs_shard(target, ctx)
    if ocaml_shard:
        parts.append({"dx_results": [ocaml_shard], "dx_docs_shards": [ocaml_shard]})
    ts_shard = _typescript_docs_shard(target, ctx)
    if ts_shard:
        parts.append({"dx_results": [ts_shard], "dx_docs_shards": [ts_shard]})
    js_shard = _javascript_docs_shard(target, ctx)
    if js_shard:
        parts.append({"dx_results": [js_shard], "dx_docs_shards": [js_shard]})
    component_shard = _component_docs_shard(target, ctx)
    if component_shard:
        parts.append({"dx_results": [component_shard], "dx_docs_shards": [component_shard]})
    if not parts:
        return []
    groups = {}
    for part in parts:
        for name, files in part.items():
            groups[name] = groups.get(name, []) + files
    return [OutputGroupInfo(**{name: depset(files) for name, files in groups.items()})]

real_docs_aspect = aspect(
    implementation = _real_docs_aspect_impl,
    doc = "Puts the docs sites, shards and API trees of a target in dx_results and a manifest naming each site tree in dx_docs_sites. Rust, Go, Python, JVM, D, Dart, C#, F#, PowerShell, Ruby, Julia, OCaml, TypeScript, JavaScript, Vue, Svelte, Astro and MDX library shards. The Ruby branch runs the pinned YARD (@dx_yard) and the DocIr extractor (//tools/docs/ruby/extract:ruby_extract) over a library's own .rb srcs go in dx_docs_shards. The TypeScript branch runs the pinned TypeDoc (//quality/tools/javascript/bin:typedoc) and the DocIr extractor (//tools/docs/typescript/extract:typescript_extract) over each non-test ts_project. The JavaScript branch runs the same TypeDoc over a js_library of .js, .mjs, .cjs or .jsx srcs that has exactly one tsconfig.json (with allowJs) in its package. Rust shards also put their rustdoc JSON and source manifest in rustdoc_json and srcs_manifest.",
    attrs = {
        "_astro_meta": _component_meta_attr("astro"),
        "_component_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/components/extract:component_extract"),
            executable = True,
        ),
        "_csharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/csharp:csharp_api_pinned"),
            executable = True,
        ),
        "_d_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/d/extract:d_extract"),
            executable = True,
        ),
        "_dotnet_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/extract:dotnet_extract"),
            executable = True,
        ),
        "_dart_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dart/extract:dart_extract"),
            executable = True,
        ),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/godoc:godoc_json"),
            executable = True,
        ),
        "_error_format": attr.label(default = Label("@rules_rust//rust/settings:error_format")),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
        "_fsharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/fsharp:fsharp_api_pinned"),
            executable = True,
        ),
        "_go_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/extract:go_extract"),
            executable = True,
        ),
        "_mdx_meta": _component_meta_attr("mdx"),
        "_process_wrapper": attr.label(
            default = Label("@rules_rust//util/process_wrapper"),
            cfg = "exec",
            executable = True,
        ),
        "_java_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/javadoc:javadoc_extract"),
            executable = True,
        ),
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/convert:jvm_convert"),
            executable = True,
        ),
        "_kotlin_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/dokka:dokka_extract"),
            executable = True,
        ),
        "_pwsh": attr.label(
            cfg = "exec",
            default = Label("@rules_powershell//powershell:current_pwsh_toolchain"),
        ),
        "_pwsh_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/powershell/extract:powershell_extract"),
            executable = True,
        ),
        "_pwsh_script": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/powershell:extract.ps1"),
        ),
        "_py_driver": attr.label(
            cfg = "exec",
            default = Label("//quality/tools/python:griffe_dump"),
            executable = True,
        ),
        "_py_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/python/extract:python_extract"),
            executable = True,
        ),
        "_scala_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/scaladoc:scaladoc_extract"),
            executable = True,
        ),
        "_julia_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/julia:doc_dump"),
            executable = True,
        ),
        "_julia_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/julia/extract:julia_extract"),
            executable = True,
        ),
        "_odoc": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml:odoc_tool"),
            executable = True,
        ),
        "_ocaml_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml/extract:ocaml_extract"),
            executable = True,
        ),
        "_ruby_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/ruby:yard_dump.rb"),
        ),
        "_ruby_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ruby/extract:ruby_extract"),
            executable = True,
        ),
        "_ruby_yard": attr.label(
            default = Label("@dx_yard//:lib"),
        ),
        "_ts_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/typescript/extract:typescript_extract"),
            executable = True,
        ),
        "_typedoc": attr.label(
            cfg = "exec",
            default = Label("//quality/tools/javascript/bin:typedoc"),
            executable = True,
        ),
        "_svelte_meta": _component_meta_attr("svelte"),
        "_vue_meta": _component_meta_attr("vue"),
    },
    fragments = ["cpp"],
    requires = [rust_doc_inputs_aspect, dotnet_docs_collect_aspect],
    toolchains = [
        config_common.toolchain_type(_DART_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(D_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(KOTLIN_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_RUBY_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_RUST_DOCS_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_CC_DOCS_TOOLCHAIN, mandatory = False),
    ],
)
