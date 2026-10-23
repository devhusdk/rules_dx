"""Target-scoped real capability aspects over real adapters."""

load("@aspect_rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_proto//proto:defs.bzl", "ProtoInfo")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", _rust_common = "rust_common")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load(
    "//quality:adapters.bzl",
    "REAL_ADAPTERS",
    "REAL_CLASS_TO_FAMILY",
)
load("//libs/starlark:resources.bzl", "dx_quality_resources")
load("//quality:execution_requirements.bzl", "dx_stage_execution_requirements")
load("//quality:native_config.bzl", "DxNativeConfigInfo", "collect_native_configs", "missing_required_config_error")
load("//quality:parity_tests.bzl", "deferred_pipeline_error")
load("//quality:pipeline.bzl", "aspect_capability_blocked", "aspect_direct_maps", "aspect_family_selections", "drop_pipeline_tool", "filter_pipeline_by_tools", "generated_source_paths", "ordered_pipeline_paths", "pipeline_inputs_for_paths", "prune_tool_generated_sources", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline")
load("//quality:policy.bzl", "QualityPolicyInfo", "family_section_error")
load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources", "upstream_source_class")
load("//rust/rules:edition.bzl", "RUST_EDITION")
load("//rust/toolchains:bindings.bzl", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")

_REAL_TOOL_TABLE = {
    "biome": {"capabilities": ["format", "lint"], "shard": "core"},
    "buf": {"capabilities": ["format", "lint"], "shard": "protobuf"},
    "buildifier": {"capabilities": ["format", "lint"], "shard": "core"},
    "checkstyle": {"capabilities": ["lint"], "shard": "jvm"},
    "clang_format": {"capabilities": ["format"], "shard": "cc"},
    "clang_tidy": {"capabilities": ["lint"], "shard": "cc"},
    "clippy": {"capabilities": ["lint"], "shard": "rust"},
    "csharpier": {"capabilities": ["format"], "shard": "dotnet"},
    "erlfmt": {"capabilities": ["format"], "shard": "core"},
    "eslint": {"capabilities": ["lint"], "shard": "js"},
    "fantomas": {"capabilities": ["format"], "shard": "dotnet"},
    "flake8": {"capabilities": ["lint"], "shard": "py"},
    "fsharplint": {"capabilities": ["lint"], "shard": "dotnet"},
    "gleam_format": {"capabilities": ["format"], "shard": "core"},
    "google_java_format": {"capabilities": ["format"], "shard": "jvm"},
    "keep_sorted": {"capabilities": ["lint"], "shard": "text"},
    "ktfmt": {"capabilities": ["format"], "shard": "jvm"},
    "ktlint": {"capabilities": ["lint"], "shard": "jvm"},
    "markdown_check": {"capabilities": ["lint"], "shard": "core"},
    "mix_format": {"capabilities": ["format"], "shard": "core"},
    "pmd": {"capabilities": ["lint"], "shard": "jvm"},
    "prettier": {"capabilities": ["format"], "shard": "js"},
    "pydoclint": {"capabilities": ["lint"], "shard": "py"},
    "pylint": {"capabilities": ["lint"], "shard": "py"},
    "qmlformat": {"capabilities": ["format"], "shard": "core"},
    "qmllint": {"capabilities": ["lint"], "shard": "core"},
    "ruff": {"capabilities": ["format", "lint"], "shard": "core"},
    "rustc": {"capabilities": ["typecheck"], "shard": "rust"},
    "rustfmt": {"capabilities": ["format"], "shard": "rust"},
    "scalafix": {"capabilities": ["lint"], "shard": "scala"},
    "scalafmt": {"capabilities": ["format"], "shard": "scala"},
    "shellcheck": {"capabilities": ["lint"], "shard": "shell"},
    "shfmt": {"capabilities": ["format"], "shard": "shell"},
    "spotbugs": {"capabilities": ["lint"], "shard": "jvm"},
    "taplo": {"capabilities": ["format", "lint"], "shard": "core"},
    "terraform": {"capabilities": ["format"], "shard": "terraform"},
    "ty": {"capabilities": ["typecheck"], "shard": "core"},
    "vale": {"capabilities": ["lint"], "shard": "core"},
    "zig_fmt": {"capabilities": ["format"], "shard": "core"},
}

def _shard_tools(shard, capability):
    """Lists wired tools for one shard/capability from the per-tool table."""
    return sorted([tool for tool in _REAL_TOOL_TABLE if _REAL_TOOL_TABLE[tool]["shard"] == shard and capability in _REAL_TOOL_TABLE[tool]["capabilities"]])

_CORE_LINT_TOOLS = _shard_tools("core", "lint")
_CORE_FORMAT_TOOLS = _shard_tools("core", "format")
_CORE_TYPECHECK_TOOLS = _shard_tools("core", "typecheck")
_CC_FORMAT_TOOLS = _shard_tools("cc", "format")
_CC_LINT_TOOLS = _shard_tools("cc", "lint")
_JS_LINT_TOOLS = _shard_tools("js", "lint")
_JS_FORMAT_TOOLS = _shard_tools("js", "format")
_PY_LINT_TOOLS = _shard_tools("py", "lint")
_RUST_LINT_TOOLS = _shard_tools("rust", "lint")
_RUST_FORMAT_TOOLS = _shard_tools("rust", "format")
_RUST_TYPECHECK_TOOLS = _shard_tools("rust", "typecheck")
_SCALA_FORMAT_TOOLS = _shard_tools("scala", "format")
_SCALA_LINT_TOOLS = _shard_tools("scala", "lint")
_SHELL_LINT_TOOLS = _shard_tools("shell", "lint")
_SHELL_FORMAT_TOOLS = _shard_tools("shell", "format")
_TEXT_LINT_TOOLS = _shard_tools("text", "lint")
_TERRAFORM_FORMAT_TOOLS = _shard_tools("terraform", "format")
_PROTOBUF_FORMAT_TOOLS = _shard_tools("protobuf", "format")
_PROTOBUF_LINT_TOOLS = _shard_tools("protobuf", "lint")
_JVM_LINT_TOOLS = _shard_tools("jvm", "lint")
_JVM_FORMAT_TOOLS = _shard_tools("jvm", "format")
_DOTNET_FORMAT_TOOLS = _shard_tools("dotnet", "format")
_DOTNET_LINT_TOOLS = _shard_tools("dotnet", "lint")

_WIRED_TOOLS = {
    "format": sorted(set(_CC_FORMAT_TOOLS + _CORE_FORMAT_TOOLS + _JS_FORMAT_TOOLS + _JVM_FORMAT_TOOLS + _RUST_FORMAT_TOOLS + _SCALA_FORMAT_TOOLS + _SHELL_FORMAT_TOOLS + _TERRAFORM_FORMAT_TOOLS + _DOTNET_FORMAT_TOOLS)),
    "lint": sorted(set(_CC_LINT_TOOLS + _CORE_LINT_TOOLS + _DOTNET_LINT_TOOLS + _JS_LINT_TOOLS + _JVM_LINT_TOOLS + _PY_LINT_TOOLS + _RUST_LINT_TOOLS + _SCALA_LINT_TOOLS + _SHELL_LINT_TOOLS + _TEXT_LINT_TOOLS)),
    "format": sorted(set(_CC_FORMAT_TOOLS + _CORE_FORMAT_TOOLS + _JS_FORMAT_TOOLS + _JVM_FORMAT_TOOLS + _RUST_FORMAT_TOOLS + _SCALA_FORMAT_TOOLS + _SHELL_FORMAT_TOOLS + _TERRAFORM_FORMAT_TOOLS + _DOTNET_FORMAT_TOOLS + _PROTOBUF_FORMAT_TOOLS)),
    "lint": sorted(set(_CC_LINT_TOOLS + _CORE_LINT_TOOLS + _JS_LINT_TOOLS + _JVM_LINT_TOOLS + _PY_LINT_TOOLS + _RUST_LINT_TOOLS + _SCALA_LINT_TOOLS + _SHELL_LINT_TOOLS + _TEXT_LINT_TOOLS + _PROTOBUF_LINT_TOOLS)),
    "typecheck": sorted(set(_CORE_TYPECHECK_TOOLS + _RUST_TYPECHECK_TOOLS)),
}

def real_policy_error(policy, capability, what):
    """Validates the selected workspace policy for one real capability."""
    for family_id in sorted(policy.families.keys()):
        section = policy.families[family_id]
        error = family_section_error(
            family_id,
            capability,
            getattr(section, capability),
            section.disabled,
            _WIRED_TOOLS[capability],
        )
        if error != "":
            return what + ": " + error
    return ""

def upstream_direct_sources(target, ctx):
    """Derives directly owned sources for an ordinary upstream native target."""
    is_rust = _rust_common.crate_info in target or _rust_common.test_crate_info in target
    is_cc = CcInfo in target
    is_proto = ProtoInfo in target
    if not is_rust and not is_cc and not is_proto:
        return (False, {}, "")
    files = []
    if is_proto:
        proto_srcs = getattr(ctx.rule.files, "srcs", None)
        if proto_srcs == None:
            return (True, {}, "upstream proto target has no readable srcs; unsupported ownership is unchecked, not clean")
        files.extend(proto_srcs)
    if is_rust:
        rust_srcs = getattr(ctx.rule.files, "srcs", None)
        if rust_srcs == None:
            return (True, {}, "upstream rust target has no readable srcs; unsupported ownership is unchecked, not clean")
        files.extend(rust_srcs)
    if is_cc:
        cc_srcs = getattr(ctx.rule.files, "srcs", None)
        cc_hdrs = getattr(ctx.rule.files, "hdrs", None)
        if cc_srcs == None and cc_hdrs == None:
            return (True, {}, "upstream cc target has no readable srcs or hdrs; unsupported ownership is unchecked, not clean")
        if cc_srcs != None:
            files.extend(cc_srcs)
        if cc_hdrs != None:
            files.extend(cc_hdrs)
    buckets = {}
    for f in files:
        if not f.is_source:
            continue
        class_id = upstream_source_class(f.extension)
        if class_id == "":
            continue
        buckets.setdefault(class_id, []).append(f)
    direct_sources = {}
    for class_id in buckets.keys():
        if len(buckets[class_id]) > 0:
            direct_sources[class_id] = depset(buckets[class_id])
    check_direct_sources(direct_sources, str(target.label))
    return (True, direct_sources, "")

_QML_CONTEXT_EXTENSIONS = ["qml", "qmltypes", "js", "mjs"]

QmlContextInfo = provider(
    doc = "QML import context files reachable through deps.",
    fields = {"files": "depset[File]: qmldir, qml, qmltypes and js files."},
)

def _qml_context_impl(target, ctx):
    direct = [
        f
        for f in target[DefaultInfo].files.to_list()
        if f.basename == "qmldir" or f.extension in _QML_CONTEXT_EXTENSIONS
    ]
    transitive = [
        dep[QmlContextInfo].files
        for dep in getattr(ctx.rule.attr, "deps", [])
        if type(dep) == "Target" and QmlContextInfo in dep
    ]
    return [QmlContextInfo(files = depset(direct, transitive = transitive))]

_qml_context_aspect = aspect(
    implementation = _qml_context_impl,
    attr_aspects = ["deps"],
    provides = [QmlContextInfo],
)

def _scalafix_context(target, what):
    """Returns the semanticdb jars and the dependency jars scalafix reads for one target."""
    if SemanticdbInfo not in target or not target[SemanticdbInfo].semanticdb_enabled or not target[SemanticdbInfo].is_bundled_in_jar:
        fail(what + ": scalafix needs semanticdb bundled in the jar; add register_toolchains(\"@rules_dx//scala/toolchain:semanticdb\") to MODULE.bazel, or set enable_semanticdb = True and semanticdb_bundle_in_jar = True on your own scala_toolchain")
    if JavaInfo not in target:
        fail(what + ": scalafix needs a JavaInfo target")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None:
            owned[output.class_jar.path] = output.class_jar
    if len(owned) == 0:
        fail(what + ": scalafix found no compiled jar for this target")
    auxiliary = {}
    for jar in target[JavaInfo].transitive_runtime_jars.to_list():
        if jar.path not in owned:
            auxiliary[jar.path] = jar
    return (
        sorted(owned.values(), key = lambda f: f.path),
        sorted(auxiliary.values(), key = lambda f: f.path),
    )

def _fsharplint_context(target, ctx, what):
    """Returns the dependency assemblies and the compile context file for one F# target."""
    if DotnetAssemblyRuntimeInfo not in target:
        fail(what + ": fsharplint needs the compiled dependency assemblies of this target")
    if not hasattr(ctx.rule.attr, "defines"):
        fail(what + ": fsharplint needs an fsharp_* target with compile symbols")
    runtime = target[DotnetAssemblyRuntimeInfo]
    seen = {lib.basename: True for lib in runtime.libs}
    refs = []
    for info in runtime.deps.to_list():
        for lib in info.libs:
            if lib.basename.endswith(".dll") and lib.basename not in seen:
                seen[lib.basename] = True
                refs.append(lib)
    context = ctx.actions.declare_file(target.label.name + "-fsharplint-context.json")
    ctx.actions.write(
        output = context,
        content = json.encode({
            "defines": list(ctx.rule.attr.defines),
            "langversion": ctx.rule.attr.langversion,
            "refs": ["dx-fsharplint/ref/%d-%s" % (index, lib.basename) for index, lib in enumerate(refs)],
            "sources": [f.short_path for f in ctx.rule.files.srcs],
        }),
    )
    return (refs, context)

def _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain):
    what = "real_aspect (" + str(target.label) + ")"
    policy = ctx.attr._policy[QualityPolicyInfo]
    policy_error = real_policy_error(policy, capability, what)
    if policy_error != "":
        fail(policy_error)
    if aspect_capability_blocked(ctx.rule.attr, capability):
        return []
    if QualitySourcesInfo in target:
        direct_sources = target[QualitySourcesInfo].direct_sources
    else:
        (recognized, derived, ownership_error) = upstream_direct_sources(target, ctx)
        if ownership_error != "":
            fail(what + ": " + ownership_error)
        if not recognized:
            return []
        if len(derived) == 0:
            return []
        direct_sources = derived

    (target_classes, direct_files, direct_paths, path_to_file) = aspect_direct_maps(direct_sources, what)
    selections = aspect_family_selections(policy, capability)
    resolved = resolve_pipeline(
        target_classes,
        direct_paths,
        capability,
        selections,
        REAL_CLASS_TO_FAMILY,
        REAL_ADAPTERS,
    )
    if len(resolved) == 0:
        err = deferred_pipeline_error(target_classes, capability)
        if err != "":
            fail(what + ": " + err)
        return []
    resolved = filter_pipeline_by_tools(resolved, allowed_tools)
    if len(resolved) == 0:
        return []

    if "tsc" in [stage["tool"] for stage in resolved]:
        resolved = drop_pipeline_tool(resolved, "tsc")
        if len(resolved) == 0:
            return []

    spotbugs_owned = []
    spotbugs_aux = []
    if "spotbugs" in [stage["tool"] for stage in resolved]:
        if JavaInfo not in target:
            resolved = drop_pipeline_tool(resolved, "spotbugs")
            if len(resolved) == 0:
                return []
        else:
            owned = {}
            for output in target[JavaInfo].java_outputs:
                if output.class_jar != None and ".." not in output.class_jar.short_path.split("/"):
                    owned[output.class_jar.path] = output.class_jar
            auxiliary = {}
            for jar in target[JavaInfo].transitive_runtime_jars.to_list():
                if ".." in jar.short_path.split("/") or jar.path in owned:
                    continue
                auxiliary[jar.path] = jar
            if len(owned) == 0:
                resolved = drop_pipeline_tool(resolved, "spotbugs")
                if len(resolved) == 0:
                    return []
            else:
                spotbugs_owned = sorted(owned.values(), key = lambda f: f.short_path)
                spotbugs_aux = sorted(auxiliary.values(), key = lambda f: f.short_path)

    scalafix_owned = []
    scalafix_aux = []
    if "scalafix" in [stage["tool"] for stage in resolved]:
        (scalafix_owned, scalafix_aux) = _scalafix_context(target, what)

    fsharplint_refs = []
    fsharplint_context = None
    if "fsharplint" in [stage["tool"] for stage in resolved]:
        (fsharplint_refs, fsharplint_context) = _fsharplint_context(target, ctx, what)

    rustfmt_edition = None
    if "rustfmt" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": rustfmt needs the Rust family aspect")
        if _rust_common.crate_info in target:
            rustfmt_edition = target[_rust_common.crate_info].edition
        elif _rust_common.test_crate_info in target:
            rustfmt_edition = target[_rust_common.test_crate_info].crate.edition
        else:
            rustfmt_edition = RUST_EDITION
        generated = generated_source_paths(direct_files)
        if len(generated) > 0:
            resolved = prune_tool_generated_sources(resolved, generated, "rustfmt")
            if len(resolved) == 0:
                return []

    clippy_diagnostics = []
    if "clippy" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": clippy needs the Rust family aspect")
        if OutputGroupInfo in target and "clippy_output" in target[OutputGroupInfo]:
            clippy_diagnostics = target[OutputGroupInfo]["clippy_output"].to_list()
    clippy_delegated = len(clippy_diagnostics) > 0

    rustc_diagnostics = []
    if "rustc" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": rustc needs the Rust family aspect")
        if OutputGroupInfo in target and "rustc_output" in target[OutputGroupInfo]:
            rustc_diagnostics = target[OutputGroupInfo]["rustc_output"].to_list()
    rustc_delegated = len(rustc_diagnostics) > 0

    hints = []
    if hasattr(ctx.rule.attr, "aspect_hints"):
        for hint_target in ctx.rule.attr.aspect_hints:
            if DxNativeConfigInfo in hint_target:
                hints.append(hint_target[DxNativeConfigInfo])
    stage_tools = [stage["tool"] for stage in resolved]
    configs_by_tool = collect_native_configs(hints, stage_tools, str(target.label))

    for stage in resolved:
        config_error = missing_required_config_error(stage["tool"], configs_by_tool, str(target.label))
        if config_error != "":
            fail(config_error)

    tool_binaries = {}
    tool_extra = {}
    if "biome" in stage_tools:
        tool_binaries["biome"] = ctx.file._biome
    if "buildifier" in stage_tools:
        tool_binaries["buildifier"] = ctx.file._buildifier
    if "eslint" in stage_tools:
        tool_binaries["eslint"] = ctx.executable._eslint
        tool_extra["eslint"] = ctx.attr._eslint[DefaultInfo].files.to_list()
    if "flake8" in stage_tools:
        tool_binaries["flake8"] = ctx.executable._flake8
    if "markdown_check" in stage_tools:
        tool_binaries["markdown_check"] = ctx.file._markdown_check
    if "prettier" in stage_tools:
        tool_binaries["prettier"] = ctx.executable._prettier
        tool_extra["prettier"] = ctx.attr._prettier[DefaultInfo].files.to_list()
    if "pydoclint" in stage_tools:
        tool_binaries["pydoclint"] = ctx.executable._pydoclint
    if "pylint" in stage_tools:
        tool_binaries["pylint"] = ctx.executable._pylint
    if "qmlformat" in stage_tools:
        qmlformat = ctx.attr._qmlformat[DefaultInfo].files_to_run.executable
        if qmlformat == None:
            fail(what + ": qmlformat has no executable; set --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat")
        tool_binaries["qmlformat"] = qmlformat
    if "qmllint" in stage_tools:
        qmllint = ctx.attr._qmllint[DefaultInfo].files_to_run.executable
        if qmllint == None:
            fail(what + ": qmllint has no executable; set --@rules_dx//config:tool_qmllint=@dx_qt//:qmllint")
        tool_binaries["qmllint"] = qmllint
    if "erlfmt" in stage_tools:
        erlfmt = ctx.attr._erlfmt[DefaultInfo].files_to_run.executable
        if erlfmt == None:
            fail(what + ": erlfmt has no executable; set --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt")
        tool_binaries["erlfmt"] = erlfmt
    if "gleam_format" in stage_tools:
        gleam_format = ctx.attr._gleam_format[DefaultInfo].files_to_run.executable
        if gleam_format == None:
            fail(what + ": gleam_format has no executable; set --@rules_dx//config:tool_gleam_format=@rules_dx//gleam/tools:gleam_format")
        tool_binaries["gleam_format"] = gleam_format
    if "mix_format" in stage_tools:
        mix_format = ctx.attr._mix_format[DefaultInfo].files_to_run.executable
        if mix_format == None:
            fail(what + ": mix_format has no executable; set --@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format")
        tool_binaries["mix_format"] = mix_format
    if "zig_fmt" in stage_tools:
        zig_fmt = ctx.attr._zig_fmt[DefaultInfo].files_to_run.executable
        if zig_fmt == None:
            fail(what + ": zig_fmt has no executable; set --@rules_dx//config:tool_zig_fmt=@rules_dx//zig/tools:zig")
        tool_binaries["zig_fmt"] = zig_fmt
    if "ruff" in stage_tools:
        tool_binaries["ruff"] = ctx.file._ruff
    if "csharpier" in stage_tools:
        tool_binaries["csharpier"] = ctx.executable._csharpier
    if "fantomas" in stage_tools:
        tool_binaries["fantomas"] = ctx.executable._fantomas
    if "fsharplint" in stage_tools:
        tool_binaries["fsharplint"] = ctx.executable._fsharplint
    if "scalafix" in stage_tools:
        tool_binaries["scalafix"] = ctx.executable._scalafix
    if "scalafmt" in stage_tools:
        tool_binaries["scalafmt"] = ctx.file._scalafmt
    if "shellcheck" in stage_tools:
        tool_binaries["shellcheck"] = ctx.file._shellcheck
    if "shfmt" in stage_tools:
        tool_binaries["shfmt"] = ctx.file._shfmt
    if "keep_sorted" in stage_tools:
        tool_binaries["keep_sorted"] = ctx.file._keep_sorted
    if "taplo" in stage_tools:
        tool_binaries["taplo"] = ctx.file._taplo
    if "terraform" in stage_tools:
        tool_binaries["terraform"] = ctx.file._terraform
    if "buf" in stage_tools:
        tool_binaries["buf"] = ctx.file._buf
    if "ty" in stage_tools:
        tool_binaries["ty"] = ctx.file._ty
    if "vale" in stage_tools:
        tool_binaries["vale"] = ctx.file._vale
    if "google_java_format" in stage_tools:
        tool_binaries["google_java_format"] = ctx.executable._google_java_format
    if "ktfmt" in stage_tools:
        tool_binaries["ktfmt"] = ctx.executable._ktfmt
    if "checkstyle" in stage_tools:
        tool_binaries["checkstyle"] = ctx.executable._checkstyle
    if "clang_format" in stage_tools:
        tool_binaries["clang_format"] = ctx.file._clang_format
    if "clang_tidy" in stage_tools:
        tool_binaries["clang_tidy"] = ctx.file._clang_tidy
    if "pmd" in stage_tools:
        tool_binaries["pmd"] = ctx.executable._pmd
    if "spotbugs" in stage_tools:
        tool_binaries["spotbugs"] = ctx.executable._spotbugs
    if "ktlint" in stage_tools:
        tool_binaries["ktlint"] = ctx.executable._ktlint
    if has_rust_toolchain:
        clippy_driver, rustfmt = rust_toolchain_tools(ctx)
        if "clippy" in stage_tools and not clippy_delegated:
            tool_binaries["clippy"] = clippy_driver
        if "rustfmt" in stage_tools:
            tool_binaries["rustfmt"] = rustfmt
        if "rustc" in stage_tools and not rustc_delegated:
            tool_binaries["rustc"] = rust_toolchain_rustc(ctx)

    out = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + ".pb")

    ordered_paths = ordered_pipeline_paths(resolved)
    inputs = pipeline_inputs_for_paths(ordered_paths, path_to_file)

    sibling_pairs = {}
    if "markdown_check" in stage_tools and hasattr(ctx.rule.attr, "markdown_siblings"):
        for sibling in ctx.rule.attr.markdown_siblings:
            if hasattr(sibling, "files"):
                sibling_files = sibling.files.to_list()
            else:
                sibling_files = [sibling]
            for f in sibling_files:
                if ".." in f.short_path.split("/"):
                    fail(what + ": sibling path escapes workspace: '" + f.short_path + "'")
                if f.short_path in path_to_file:
                    fail(what + ": sibling '" + f.short_path + "' shadows a checked source; siblings must not shadow sources")
                if f.short_path in sibling_pairs:
                    fail(what + ": duplicate sibling '" + f.short_path + "'; siblings must be unique")
                sibling_pairs[f.short_path] = f

    request_stages = [real_request_stage(stage["tool"], stage["classes"], stage["sources"]) for stage in resolved]
    request_sources = []
    for ws_path in ordered_paths:
        f = path_to_file.get(ws_path)
        if f != None:
            request_sources.append(real_request_mapping(ws_path, f.path))
    request_siblings = []
    for ws_path in sorted(sibling_pairs.keys()):
        f = sibling_pairs[ws_path]
        request_siblings.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    resolve_pairs = {}
    if "ty" in stage_tools and _PyInfo in target:
        transitive = getattr(target[_PyInfo], "transitive_sources", None)
        if transitive != None:
            for f in transitive.to_list():
                ws_path = f.short_path

                if ".." in ws_path.split("/"):
                    continue
                if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                    continue
                if not ws_path.endswith(".py") and not ws_path.endswith(".pyi"):
                    continue
                resolve_pairs[ws_path] = f
    ty_search_dirs = []
    if "ty" in stage_tools and _PyInfo in target:
        transitive_dirs = getattr(target[_PyInfo], "transitive_sources", None)
        if transitive_dirs != None:
            for f in transitive_dirs.to_list():
                if f.is_directory and f.short_path.endswith(".install"):
                    ty_search_dirs.append(f)
                    inputs.append(f)
    if "qmllint" in stage_tools:
        context_files = list(target[DefaultInfo].files.to_list())
        if QmlContextInfo in target:
            context_files.extend(target[QmlContextInfo].files.to_list())
        for f in context_files:
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = "_external/" + ws_path[3:]
            if ".." in ws_path.split("/"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            if f.basename != "qmldir" and f.extension not in _QML_CONTEXT_EXTENSIONS:
                continue
            resolve_pairs[ws_path] = f
    if "buf" in stage_tools and ProtoInfo in target:
        proto_roots = sorted([r for r in target[ProtoInfo].transitive_proto_path.to_list() if r != "."], key = lambda r: -len(r))
        for f in target[ProtoInfo].transitive_sources.to_list():
            ws_path = f.path
            for root in proto_roots:
                if ws_path.startswith(root + "/"):
                    ws_path = ws_path[len(root) + 1:]
                    break
            if ".." in ws_path.split("/"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    request_resolves = []
    for ws_path in sorted(resolve_pairs.keys()):
        f = resolve_pairs[ws_path]
        request_resolves.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    tool_env_entries = {}
    run_tools = []
    if "eslint" in stage_tools:
        run_tools.append(ctx.attr._eslint[DefaultInfo].files_to_run)
        tool_env_entries["eslint"] = [real_request_tool_env("JS_BINARY__NO_CD_BINDIR", "1")]
    if "prettier" in stage_tools:
        run_tools.append(ctx.attr._prettier[DefaultInfo].files_to_run)
        tool_env_entries["prettier"] = [real_request_tool_env("JS_BINARY__NO_CD_BINDIR", "1")]
    if "pydoclint" in stage_tools:
        run_tools.append(ctx.attr._pydoclint[DefaultInfo].files_to_run)
        tool_env_entries["pydoclint"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]
    if "flake8" in stage_tools:
        run_tools.append(ctx.attr._flake8[DefaultInfo].files_to_run)
        tool_env_entries["flake8"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]
    if "pylint" in stage_tools:
        run_tools.append(ctx.attr._pylint[DefaultInfo].files_to_run)
        tool_env_entries["pylint"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]
    if "buf" in stage_tools:
        tool_env_entries["buf"] = [real_request_tool_env("PATH", "/usr/bin:/bin")]
    if "terraform" in stage_tools:
        tool_env_entries["terraform"] = [
            real_request_tool_env("CHECKPOINT_DISABLE", "1"),
            real_request_tool_env("TF_IN_AUTOMATION", "1"),
            real_request_tool_env("TF_INPUT", "0"),
        ]

    if "qmlformat" in stage_tools:
        run_tools.append(ctx.attr._qmlformat[DefaultInfo].files_to_run)
    if "qmllint" in stage_tools:
        run_tools.append(ctx.attr._qmllint[DefaultInfo].files_to_run)
    if "zig_fmt" in stage_tools:
        run_tools.append(ctx.attr._zig_fmt[DefaultInfo].files_to_run)
    if "erlfmt" in stage_tools:
        run_tools.append(ctx.attr._erlfmt[DefaultInfo].files_to_run)
    if "gleam_format" in stage_tools:
        run_tools.append(ctx.attr._gleam_format[DefaultInfo].files_to_run)
    if "mix_format" in stage_tools:
        run_tools.append(ctx.attr._mix_format[DefaultInfo].files_to_run)
    if "google_java_format" in stage_tools:
        run_tools.append(ctx.attr._google_java_format[DefaultInfo].files_to_run)
    if "ktfmt" in stage_tools:
        run_tools.append(ctx.attr._ktfmt[DefaultInfo].files_to_run)
    if "checkstyle" in stage_tools:
        run_tools.append(ctx.attr._checkstyle[DefaultInfo].files_to_run)
    if "pmd" in stage_tools:
        run_tools.append(ctx.attr._pmd[DefaultInfo].files_to_run)
    if "csharpier" in stage_tools:
        run_tools.append(ctx.attr._csharpier[DefaultInfo].files_to_run)
    if "fantomas" in stage_tools:
        run_tools.append(ctx.attr._fantomas[DefaultInfo].files_to_run)
    if "fsharplint" in stage_tools:
        run_tools.append(ctx.attr._fsharplint[DefaultInfo].files_to_run)
    if "scalafix" in stage_tools:
        run_tools.append(ctx.attr._scalafix[DefaultInfo].files_to_run)
    if "spotbugs" in stage_tools:
        run_tools.append(ctx.attr._spotbugs[DefaultInfo].files_to_run)
    if "ktlint" in stage_tools:
        run_tools.append(ctx.attr._ktlint[DefaultInfo].files_to_run)

    request_tools = {}
    for tool in stage_tools:
        binary_path = None
        if not (tool == "clippy" and clippy_delegated) and not (tool == "rustc" and rustc_delegated):
            binary = tool_binaries[tool]
            binary_path = binary.path
            inputs.append(binary)
            inputs.extend(tool_extra.get(tool, []))
        tool_files = []
        if tool == "spotbugs":
            for jar in spotbugs_owned:
                tool_files.append(real_request_tool_file("dx-spotbugs/owned/" + jar.short_path, jar.path))
                inputs.append(jar)
            for jar in spotbugs_aux:
                tool_files.append(real_request_tool_file("dx-spotbugs/aux/" + jar.short_path, jar.path))
                inputs.append(jar)
        if tool == "scalafix":
            for index, jar in enumerate(scalafix_owned):
                tool_files.append(real_request_tool_file("dx-scalafix/owned/%d-%s" % (index, jar.basename), jar.path))
                inputs.append(jar)
            for index, jar in enumerate(scalafix_aux):
                tool_files.append(real_request_tool_file("dx-scalafix/aux/%d-%s" % (index, jar.basename), jar.path))
                inputs.append(jar)
        if tool == "fsharplint":
            for index, lib in enumerate(fsharplint_refs):
                tool_files.append(real_request_tool_file("dx-fsharplint/ref/%d-%s" % (index, lib.basename), lib.path))
                inputs.append(lib)
            tool_files.append(real_request_tool_file("dx-fsharplint/context.json", fsharplint_context.path))
            inputs.append(fsharplint_context)
        edition = None
        if tool == "rustfmt":
            edition = rustfmt_edition
        config_rel = None
        if tool in configs_by_tool:
            hint = configs_by_tool[tool]
            config_rel = hint.config.short_path
            for f in sorted(hint.closure.to_list(), key = lambda f: f.short_path):
                tool_files.append(real_request_tool_file(f.short_path, f.path))
                inputs.append(f)
        upstream = []
        if tool == "clippy" and clippy_delegated:
            for diagnostics in clippy_diagnostics:
                upstream.append(diagnostics.path)
                inputs.append(diagnostics)
        if tool == "rustc" and rustc_delegated:
            for diagnostics in rustc_diagnostics:
                upstream.append(diagnostics.path)
                inputs.append(diagnostics)
        request_tools[tool] = real_request_tool(
            binary_path = binary_path,
            config_rel = config_rel,
            edition = edition,
            files = tool_files,
            env = tool_env_entries.get(tool, []),
            upstream = upstream,
            search_paths = [f.path for f in ty_search_dirs] if tool == "ty" else [],
        )

    doc = real_request_doc(
        str(target.label),
        capability,
        request_stages,
        request_sources,
        request_siblings,
        request_resolves,
        request_tools,
    )
    request_file = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + "-request.json")
    ctx.actions.write(output = request_file, content = json.encode(doc))
    inputs.append(request_file)

    args = ctx.actions.args()
    args.add("--request", request_file.path)
    args.add("--output", out.path)

    ctx.actions.run(
        executable = ctx.executable._runner,
        inputs = depset(inputs),
        tools = run_tools,
        outputs = [out],
        arguments = [args],
        execution_requirements = dx_stage_execution_requirements(stage_tools, capability),
        mnemonic = "DxRealQuality" + capability.capitalize(),
        resource_set = dx_quality_resources,
        progress_message = "Dx real quality " + capability + " %{label}",
    )

    return [OutputGroupInfo(dx_results = depset([out]))]

def _make_real_impl(capability, allowed_tools, output_suffix, has_rust_toolchain):
    """Makes one shard impl over the shared real pipeline action."""

    def _impl(target, ctx):
        return _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain)

    return _impl

def real_wired_tools():
    """Returns the sorted tool IDs the real aspects can run."""
    return sorted(_REAL_TOOL_TABLE.keys())

def real_allowed_tools_error():
    """Validates aspect shards stay registry subsets."""
    allowed = (
        _CORE_LINT_TOOLS + _CORE_FORMAT_TOOLS + _CORE_TYPECHECK_TOOLS +
        _JS_LINT_TOOLS + _JS_FORMAT_TOOLS + _PY_LINT_TOOLS +
        _RUST_LINT_TOOLS + _RUST_FORMAT_TOOLS + _RUST_TYPECHECK_TOOLS +
        _SCALA_FORMAT_TOOLS + _SCALA_LINT_TOOLS + _SHELL_LINT_TOOLS + _SHELL_FORMAT_TOOLS + _TEXT_LINT_TOOLS + _TERRAFORM_FORMAT_TOOLS + _DOTNET_FORMAT_TOOLS + _DOTNET_LINT_TOOLS + _PROTOBUF_FORMAT_TOOLS + _PROTOBUF_LINT_TOOLS +
        _JVM_LINT_TOOLS + _JVM_FORMAT_TOOLS + _CC_FORMAT_TOOLS + _CC_LINT_TOOLS
    )
    for tool in allowed:
        if tool not in REAL_ADAPTERS:
            return "real aspects: allowed tool '" + tool + "' is outside REAL_ADAPTERS"
    for tool in _REAL_TOOL_TABLE:
        if tool not in REAL_ADAPTERS:
            return "real aspects: table tool '" + tool + "' is outside REAL_ADAPTERS"
        for capability in _REAL_TOOL_TABLE[tool]["capabilities"]:
            if capability not in REAL_ADAPTERS[tool]:
                return "real aspects: table tool '" + tool + "' names unwired capability '" + capability + "'"
    return ""

_REAL_BASE_ATTRS = {
    "_policy": attr.label(
        default = "//config:workspace",
        providers = [QualityPolicyInfo],
    ),
    "_runner": attr.label(
        default = "//quality/runner:quality_runner",
        executable = True,
        cfg = "exec",
        allow_files = True,
    ),
}

_REAL_TOOL_ATTR_DEFS = {
    "biome": attr.label(
        default = "@dx_tools//:biome",
        allow_single_file = True,
        cfg = "exec",
    ),
    "buildifier": attr.label(
        default = "@dx_tools//:buildifier",
        allow_single_file = True,
        cfg = "exec",
    ),
    "checkstyle": attr.label(
        default = "//quality/tools/jvm:checkstyle",
        cfg = "exec",
        executable = True,
    ),
    "clang_format": attr.label(
        default = "@llvm//tools:clang-format",
        allow_single_file = True,
        cfg = "exec",
    ),
    "clang_tidy": attr.label(
        default = "@llvm//tools:clang-tidy",
        allow_single_file = True,
        cfg = "exec",
    ),
    "eslint": attr.label(
        default = "//quality/tools/javascript/bin:eslint",
        cfg = "exec",
        executable = True,
    ),
    "flake8": attr.label(
        default = "//quality/tools/python:flake8",
        cfg = "exec",
        executable = True,
    ),
    "google_java_format": attr.label(
        default = "//quality/tools/jvm:google_java_format",
        cfg = "exec",
        executable = True,
    ),
    "ktfmt": attr.label(
        default = "//quality/tools/jvm:ktfmt",
        cfg = "exec",
        executable = True,
    ),
    "ktlint": attr.label(
        default = "//quality/tools/jvm:ktlint",
        cfg = "exec",
        executable = True,
    ),
    "markdown_check": attr.label(
        default = "//quality/markdown:quality_markdown",
        allow_single_file = True,
        cfg = "exec",
    ),
    "pmd": attr.label(
        default = "//quality/tools/jvm:pmd",
        cfg = "exec",
        executable = True,
    ),
    "prettier": attr.label(
        default = "//quality/tools/javascript/bin:prettier",
        cfg = "exec",
        executable = True,
    ),
    "pydoclint": attr.label(
        default = "//quality/tools/python:pydoclint",
        cfg = "exec",
        executable = True,
    ),
    "pylint": attr.label(
        default = "//quality/tools/python:pylint",
        cfg = "exec",
        executable = True,
    ),
    "qmlformat": attr.label(
        default = "//config:tool_qmlformat",
        allow_files = True,
        cfg = "exec",
    ),
    "qmllint": attr.label(
        default = "//config:tool_qmllint",
        allow_files = True,
        cfg = "exec",
    ),
    "zig_fmt": attr.label(
        default = "//config:tool_zig_fmt",
        allow_files = True,
        cfg = "exec",
    ),
    "erlfmt": attr.label(
        default = "//config:tool_erlfmt",
        allow_files = True,
        cfg = "exec",
    ),
    "gleam_format": attr.label(
        default = "//config:tool_gleam_format",
        allow_files = True,
        cfg = "exec",
    ),
    "mix_format": attr.label(
        default = "//config:tool_mix_format",
        allow_files = True,
        cfg = "exec",
    ),
    "ruff": attr.label(
        default = "//config:tool_ruff",
        allow_single_file = True,
        cfg = "exec",
    ),
    "csharpier": attr.label(
        default = "//quality/tools/dotnet:csharpier",
        cfg = "exec",
        executable = True,
    ),
    "fantomas": attr.label(
        default = "//quality/tools/dotnet:fantomas",
        cfg = "exec",
        executable = True,
    ),
    "fsharplint": attr.label(
        default = "//quality/tools/dotnet/fsharplint",
        cfg = "exec",
        executable = True,
    ),
    "scalafix": attr.label(
        default = "//quality/tools/jvm:scalafix",
        cfg = "exec",
        executable = True,
    ),
    "scalafmt": attr.label(
        default = "@dx_tools//:scalafmt",
        allow_single_file = True,
        cfg = "exec",
    ),
    "shellcheck": attr.label(
        default = "@dx_tools//:shellcheck",
        allow_single_file = True,
        cfg = "exec",
    ),
    "shfmt": attr.label(
        default = "@dx_tools//:shfmt",
        allow_single_file = True,
        cfg = "exec",
    ),
    "keep_sorted": attr.label(
        default = "@dx_tools//:keep_sorted",
        allow_single_file = True,
        cfg = "exec",
    ),
    "spotbugs": attr.label(
        default = "//quality/tools/jvm:spotbugs",
        cfg = "exec",
        executable = True,
    ),
    "taplo": attr.label(
        default = "@dx_tools//:taplo",
        allow_single_file = True,
        cfg = "exec",
    ),
    "buf": attr.label(
        default = "@dx_tools//:buf",
        allow_single_file = True,
        cfg = "exec",
    ),
    "terraform": attr.label(
        default = "@dx_tools//:terraform",
        allow_single_file = True,
        cfg = "exec",
    ),
    "ty": attr.label(
        default = "@dx_tools//:ty",
        allow_single_file = True,
        cfg = "exec",
    ),
    "vale": attr.label(
        default = "@dx_tools//:vale",
        allow_single_file = True,
        cfg = "exec",
    ),
}

def _real_attrs_for(tools):
    """Builds one shard attr set from the per-tool attr table."""
    return _REAL_BASE_ATTRS | {"_" + tool: _REAL_TOOL_ATTR_DEFS[tool] for tool in tools}

_REAL_CORE_ATTRS = _real_attrs_for(sorted([tool for tool in _REAL_TOOL_TABLE if _REAL_TOOL_TABLE[tool]["shard"] == "core"]))
_REAL_CC_FORMAT_ATTRS = _real_attrs_for(_CC_FORMAT_TOOLS)
_REAL_CC_LINT_ATTRS = _real_attrs_for(_CC_LINT_TOOLS)
_REAL_DOTNET_FORMAT_ATTRS = _real_attrs_for(_DOTNET_FORMAT_TOOLS)
_REAL_DOTNET_LINT_ATTRS = _real_attrs_for(_DOTNET_LINT_TOOLS)
_REAL_JS_LINT_ATTRS = _real_attrs_for(_JS_LINT_TOOLS)
_REAL_JS_FORMAT_ATTRS = _real_attrs_for(_JS_FORMAT_TOOLS)
_REAL_JVM_LINT_ATTRS = _real_attrs_for(_JVM_LINT_TOOLS)
_REAL_JVM_FORMAT_ATTRS = _real_attrs_for(_JVM_FORMAT_TOOLS)
_REAL_PY_LINT_ATTRS = _real_attrs_for(_PY_LINT_TOOLS)
_REAL_RUST_ATTRS = _REAL_BASE_ATTRS
_REAL_SCALA_FORMAT_ATTRS = _real_attrs_for(_SCALA_FORMAT_TOOLS)
_REAL_SCALA_LINT_ATTRS = _real_attrs_for(_SCALA_LINT_TOOLS)
_REAL_SHELL_LINT_ATTRS = _real_attrs_for(_SHELL_LINT_TOOLS)
_REAL_SHELL_FORMAT_ATTRS = _real_attrs_for(_SHELL_FORMAT_TOOLS)
_REAL_TEXT_LINT_ATTRS = _real_attrs_for(_TEXT_LINT_TOOLS)
_REAL_TERRAFORM_FORMAT_ATTRS = _real_attrs_for(_TERRAFORM_FORMAT_TOOLS)
_REAL_PROTOBUF_FORMAT_ATTRS = _real_attrs_for(_PROTOBUF_FORMAT_TOOLS)
_REAL_PROTOBUF_LINT_ATTRS = _real_attrs_for(_PROTOBUF_LINT_TOOLS)

_REAL_SHARDS = {
    "real_cc_format": {"attrs": _REAL_CC_FORMAT_ATTRS, "capability": "format", "doc": "Additive C/C++ format family aspect (clang-format).", "has_rust": False, "suffix": "-cc", "tools": _CC_FORMAT_TOOLS},
    "real_cc_lint": {"attrs": _REAL_CC_LINT_ATTRS, "capability": "lint", "doc": "Additive C/C++ lint family aspect (clang-tidy opt-in).", "has_rust": False, "suffix": "-cc", "tools": _CC_LINT_TOOLS},
    "real_dotnet_format": {"attrs": _REAL_DOTNET_FORMAT_ATTRS, "capability": "format", "doc": "Additive C#/F# format family aspect (CSharpier/Fantomas).", "has_rust": False, "suffix": "-dotnet", "tools": _DOTNET_FORMAT_TOOLS},
    "real_dotnet_lint": {"attrs": _REAL_DOTNET_LINT_ATTRS, "capability": "lint", "doc": "Additive F# lint family aspect (FSharpLint).", "has_rust": False, "suffix": "-dotnet", "tools": _DOTNET_LINT_TOOLS},
    "real_format": {"attrs": _REAL_CORE_ATTRS, "capability": "format", "doc": "Registers the exact-input real format pipeline action in dx_results.", "has_rust": False, "suffix": "", "tools": _CORE_FORMAT_TOOLS},
    "real_js_format": {"attrs": _REAL_JS_FORMAT_ATTRS, "capability": "format", "doc": "Additive JavaScript/JSON format family aspect (Prettier).", "has_rust": False, "suffix": "-js", "tools": _JS_FORMAT_TOOLS},
    "real_js_lint": {"attrs": _REAL_JS_LINT_ATTRS, "capability": "lint", "doc": "Additive JavaScript lint family aspect (ESLint opt-in).", "has_rust": False, "suffix": "-js", "tools": _JS_LINT_TOOLS},
    "real_jvm_format": {"attrs": _REAL_JVM_FORMAT_ATTRS, "capability": "format", "doc": "Additive JVM format family aspect (google-java-format/ktfmt).", "has_rust": False, "suffix": "-jvm", "tools": _JVM_FORMAT_TOOLS},
    "real_jvm_lint": {"attrs": _REAL_JVM_LINT_ATTRS, "capability": "lint", "doc": "Additive JVM lint family aspect (Checkstyle/Pmd/SpotBugs/ktlint; SpotBugs target-coupled via JavaInfo).", "has_rust": False, "suffix": "-jvm", "tools": _JVM_LINT_TOOLS},
    "real_lint": {"attrs": _REAL_CORE_ATTRS, "capability": "lint", "doc": "Registers the exact-input real lint pipeline action in dx_results.", "has_rust": False, "suffix": "", "tools": _CORE_LINT_TOOLS},
    "real_protobuf_format": {"attrs": _REAL_PROTOBUF_FORMAT_ATTRS, "capability": "format", "doc": "Additive Protocol Buffers format family aspect (buf format).", "has_rust": False, "suffix": "-protobuf", "tools": _PROTOBUF_FORMAT_TOOLS},
    "real_protobuf_lint": {"attrs": _REAL_PROTOBUF_LINT_ATTRS, "capability": "lint", "doc": "Additive Protocol Buffers lint family aspect (buf lint).", "has_rust": False, "suffix": "-protobuf", "tools": _PROTOBUF_LINT_TOOLS},
    "real_python_lint": {"attrs": _REAL_PY_LINT_ATTRS, "capability": "lint", "doc": "Additive Python lint family aspect (flake8/pylint/pydoclint).", "has_rust": False, "suffix": "-py", "tools": _PY_LINT_TOOLS},
    "real_rust_format": {"attrs": _REAL_RUST_ATTRS, "capability": "format", "doc": "Additive Rust format family aspect (toolchain rustfmt).", "has_rust": True, "suffix": "-rust", "tools": _RUST_FORMAT_TOOLS},
    "real_rust_lint": {"attrs": _REAL_RUST_ATTRS, "capability": "lint", "doc": "Additive Rust lint family aspect (delegated Clippy).", "has_rust": True, "suffix": "-rust", "tools": _RUST_LINT_TOOLS},
    "real_rust_typecheck": {"attrs": _REAL_RUST_ATTRS, "capability": "typecheck", "doc": "Additive Rust typecheck family aspect (delegated rustc).", "has_rust": True, "suffix": "-rust", "tools": _RUST_TYPECHECK_TOOLS},
    "real_scala_format": {"attrs": _REAL_SCALA_FORMAT_ATTRS, "capability": "format", "doc": "Additive Scala format family aspect (scalafmt).", "has_rust": False, "suffix": "-scala", "tools": _SCALA_FORMAT_TOOLS},
    "real_scala_lint": {"attrs": _REAL_SCALA_LINT_ATTRS, "capability": "lint", "doc": "Additive Scala lint family aspect (Scalafix; target-coupled via SemanticdbInfo and JavaInfo).", "has_rust": False, "suffix": "-scala", "tools": _SCALA_LINT_TOOLS},
    "real_shell_lint": {"attrs": _REAL_SHELL_LINT_ATTRS, "capability": "lint", "doc": "Additive shell lint family aspect (ShellCheck).", "has_rust": False, "suffix": "-shell", "tools": _SHELL_LINT_TOOLS},
    "real_shell_format": {"attrs": _REAL_SHELL_FORMAT_ATTRS, "capability": "format", "doc": "Additive shell format family aspect (shfmt).", "has_rust": False, "suffix": "-shell", "tools": _SHELL_FORMAT_TOOLS},
    "real_terraform_format": {"attrs": _REAL_TERRAFORM_FORMAT_ATTRS, "capability": "format", "doc": "Additive Terraform format family aspect (terraform fmt).", "has_rust": False, "suffix": "-terraform", "tools": _TERRAFORM_FORMAT_TOOLS},
    "real_text_lint": {"attrs": _REAL_TEXT_LINT_ATTRS, "capability": "lint", "doc": "Additive text lint family aspect (keep-sorted).", "has_rust": False, "suffix": "-text", "tools": _TEXT_LINT_TOOLS},
    "real_typecheck": {"attrs": _REAL_CORE_ATTRS, "capability": "typecheck", "doc": "Registers the exact-input real typecheck pipeline action in dx_results.", "has_rust": False, "suffix": "", "tools": _CORE_TYPECHECK_TOOLS},
}

def _make_real_aspect(shard_name):
    """Builds one shard aspect from the shard table."""
    shard = _REAL_SHARDS[shard_name]
    if shard["has_rust"] and shard_name == "real_rust_lint":
        return aspect(
            implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
            attr_aspects = ["aspect_hints"],
            attrs = shard["attrs"],
            toolchains = rust_toolchain_toolchains(),
            requires = [rust_clippy_aspect],
        )
    if shard["has_rust"]:
        return aspect(
            implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
            attr_aspects = ["aspect_hints"],
            attrs = shard["attrs"],
            toolchains = rust_toolchain_toolchains(),
        )
    if "qmllint" in shard["tools"]:
        return aspect(
            implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
            attr_aspects = ["aspect_hints"],
            attrs = shard["attrs"],
            requires = [_qml_context_aspect],
            required_aspect_providers = [[QmlContextInfo]],
        )
    return aspect(
        implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
        attr_aspects = ["aspect_hints"],
        attrs = shard["attrs"],
    )

real_lint_aspect = _make_real_aspect("real_lint")

real_format_aspect = _make_real_aspect("real_format")

real_typecheck_aspect = _make_real_aspect("real_typecheck")

real_cc_format_aspect = _make_real_aspect("real_cc_format")

real_cc_lint_aspect = _make_real_aspect("real_cc_lint")

real_dotnet_format_aspect = _make_real_aspect("real_dotnet_format")

real_dotnet_lint_aspect = _make_real_aspect("real_dotnet_lint")

real_js_lint_aspect = _make_real_aspect("real_js_lint")

real_js_format_aspect = _make_real_aspect("real_js_format")

real_python_lint_aspect = _make_real_aspect("real_python_lint")

real_jvm_lint_aspect = _make_real_aspect("real_jvm_lint")

real_jvm_format_aspect = _make_real_aspect("real_jvm_format")

real_rust_lint_aspect = _make_real_aspect("real_rust_lint")

real_rust_format_aspect = _make_real_aspect("real_rust_format")

real_rust_typecheck_aspect = _make_real_aspect("real_rust_typecheck")

real_scala_format_aspect = _make_real_aspect("real_scala_format")

real_scala_lint_aspect = _make_real_aspect("real_scala_lint")

real_shell_lint_aspect = _make_real_aspect("real_shell_lint")

real_shell_format_aspect = _make_real_aspect("real_shell_format")

real_protobuf_format_aspect = _make_real_aspect("real_protobuf_format")

real_protobuf_lint_aspect = _make_real_aspect("real_protobuf_lint")

real_terraform_format_aspect = _make_real_aspect("real_terraform_format")

real_text_lint_aspect = _make_real_aspect("real_text_lint")
