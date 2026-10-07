"""Target-scoped real capability aspects over real adapters."""

load("@aspect_rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", _rust_common = "rust_common")
load(
    "//quality:adapters.bzl",
    "REAL_ADAPTERS",
    "REAL_CLASS_TO_FAMILY",
)
load("//quality:execution_requirements.bzl", "dx_execution_requirements")
load("//quality:native_config.bzl", "DxNativeConfigInfo", "collect_native_configs", "missing_required_config_error")
load("//quality:parity_tests.bzl", "deferred_pipeline_error")
load("//quality:pipeline.bzl", "aspect_capability_blocked", "aspect_direct_maps", "aspect_family_selections", "drop_pipeline_tool", "filter_pipeline_by_tools", "generated_source_paths", "ordered_pipeline_paths", "pipeline_inputs_for_paths", "prune_tool_generated_sources", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline")
load("//quality:policy.bzl", "QualityPolicyInfo", "family_section_error")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//rust/rules:edition.bzl", "RUST_EDITION")
load("//rust/toolchains:bindings.bzl", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")

_REAL_TOOL_TABLE = {
    "biome": {"capabilities": ["format", "lint"], "shard": "core"},
    "buildifier": {"capabilities": ["format", "lint"], "shard": "core"},
    "checkstyle": {"capabilities": ["lint"], "shard": "jvm"},
    "clippy": {"capabilities": ["lint"], "shard": "rust"},
    "eslint": {"capabilities": ["lint"], "shard": "js"},
    "flake8": {"capabilities": ["lint"], "shard": "py"},
    "google_java_format": {"capabilities": ["format"], "shard": "jvm"},
    "ktfmt": {"capabilities": ["format"], "shard": "jvm"},
    "ktlint": {"capabilities": ["lint"], "shard": "jvm"},
    "markdown_check": {"capabilities": ["lint"], "shard": "core"},
    "pmd": {"capabilities": ["lint"], "shard": "jvm"},
    "prettier": {"capabilities": ["format"], "shard": "js"},
    "pydoclint": {"capabilities": ["lint"], "shard": "py"},
    "pylint": {"capabilities": ["lint"], "shard": "py"},
    "ruff": {"capabilities": ["format", "lint"], "shard": "core"},
    "rustc": {"capabilities": ["typecheck"], "shard": "rust"},
    "rustfmt": {"capabilities": ["format"], "shard": "rust"},
    "spotbugs": {"capabilities": ["lint"], "shard": "jvm"},
    "taplo": {"capabilities": ["format", "lint"], "shard": "core"},
    "ty": {"capabilities": ["typecheck"], "shard": "core"},
    "vale": {"capabilities": ["lint"], "shard": "core"},
}

_JS_LAUNCH_TOOLS = ["eslint", "prettier"]

def _shard_tools(shard, capability):
    """Lists wired tools for one shard/capability from the per-tool table."""
    return sorted([tool for tool in _REAL_TOOL_TABLE if _REAL_TOOL_TABLE[tool]["shard"] == shard and capability in _REAL_TOOL_TABLE[tool]["capabilities"]])

_CORE_LINT_TOOLS = _shard_tools("core", "lint")
_CORE_FORMAT_TOOLS = _shard_tools("core", "format")
_CORE_TYPECHECK_TOOLS = _shard_tools("core", "typecheck")
_JS_LINT_TOOLS = _shard_tools("js", "lint")
_JS_FORMAT_TOOLS = _shard_tools("js", "format")
_PY_LINT_TOOLS = _shard_tools("py", "lint")
_RUST_LINT_TOOLS = _shard_tools("rust", "lint")
_RUST_FORMAT_TOOLS = _shard_tools("rust", "format")
_RUST_TYPECHECK_TOOLS = _shard_tools("rust", "typecheck")
_JVM_LINT_TOOLS = _shard_tools("jvm", "lint")
_JVM_FORMAT_TOOLS = _shard_tools("jvm", "format")

_WIRED_TOOLS = {
    "format": sorted(set(_CORE_FORMAT_TOOLS + _JS_FORMAT_TOOLS + _JVM_FORMAT_TOOLS + _RUST_FORMAT_TOOLS)),
    "lint": sorted(set(_CORE_LINT_TOOLS + _JS_LINT_TOOLS + _JVM_LINT_TOOLS + _PY_LINT_TOOLS + _RUST_LINT_TOOLS)),
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

def _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain):
    what = "real_aspect (" + str(target.label) + ")"
    policy = ctx.attr._policy[QualityPolicyInfo]
    policy_error = real_policy_error(policy, capability, what)
    if policy_error != "":
        fail(policy_error)
    if QualitySourcesInfo not in target:
        return []
    if aspect_capability_blocked(ctx.rule.attr, capability):
        return []
    info = target[QualitySourcesInfo]

    (target_classes, direct_files, direct_paths, path_to_file) = aspect_direct_maps(info.direct_sources, what)
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

    spotbugs_jars = []
    if "spotbugs" in [stage["tool"] for stage in resolved]:
        if JavaInfo not in target:
            resolved = drop_pipeline_tool(resolved, "spotbugs")
            if len(resolved) == 0:
                return []
        else:
            jars = []
            for jar in target[JavaInfo].transitive_runtime_jars.to_list():
                if ".." in jar.short_path.split("/"):
                    continue
                jars.append(jar)
            jars = sorted(jars, key = lambda f: f.short_path)
            if len(jars) == 0:
                resolved = drop_pipeline_tool(resolved, "spotbugs")
                if len(resolved) == 0:
                    return []
            else:
                spotbugs_jars = jars

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
            else:
                fail(what + ": aspect_hints must provide DxNativeConfigInfo")
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
    for js_tool in _JS_LAUNCH_TOOLS:
        if js_tool in stage_tools:
            tool_binaries[js_tool] = getattr(ctx.executable, "_" + js_tool)
            tool_extra[js_tool] = getattr(ctx.attr, "_" + js_tool)[DefaultInfo].files.to_list()
    if "flake8" in stage_tools:
        tool_binaries["flake8"] = ctx.executable._flake8
    if "markdown_check" in stage_tools:
        tool_binaries["markdown_check"] = ctx.file._markdown_check
    if "pydoclint" in stage_tools:
        tool_binaries["pydoclint"] = ctx.executable._pydoclint
    if "pylint" in stage_tools:
        tool_binaries["pylint"] = ctx.executable._pylint
    if "ruff" in stage_tools:
        tool_binaries["ruff"] = ctx.file._ruff
    if "taplo" in stage_tools:
        tool_binaries["taplo"] = ctx.file._taplo
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
    request_resolves = []
    for ws_path in sorted(resolve_pairs.keys()):
        f = resolve_pairs[ws_path]
        request_resolves.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    tool_env_entries = {}
    run_tools = []
    for js_tool in _JS_LAUNCH_TOOLS:
        if js_tool in stage_tools:
            run_tools.append(getattr(ctx.attr, "_" + js_tool)[DefaultInfo].files_to_run)
            tool_env_entries[js_tool] = [real_request_tool_env("JS_BINARY__NO_CD_BINDIR", "1")]
    if "pydoclint" in stage_tools:
        run_tools.append(ctx.attr._pydoclint[DefaultInfo].files_to_run)
        tool_env_entries["pydoclint"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]
    if "flake8" in stage_tools:
        run_tools.append(ctx.attr._flake8[DefaultInfo].files_to_run)
        tool_env_entries["flake8"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]
    if "pylint" in stage_tools:
        run_tools.append(ctx.attr._pylint[DefaultInfo].files_to_run)
        tool_env_entries["pylint"] = [real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles")]

    if "google_java_format" in stage_tools:
        run_tools.append(ctx.attr._google_java_format[DefaultInfo].files_to_run)
    if "ktfmt" in stage_tools:
        run_tools.append(ctx.attr._ktfmt[DefaultInfo].files_to_run)
    if "checkstyle" in stage_tools:
        run_tools.append(ctx.attr._checkstyle[DefaultInfo].files_to_run)
    if "pmd" in stage_tools:
        run_tools.append(ctx.attr._pmd[DefaultInfo].files_to_run)
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
            for jar in spotbugs_jars:
                tool_files.append(real_request_tool_file(jar.short_path, jar.path))
                inputs.append(jar)
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
        execution_requirements = dx_execution_requirements(),
        mnemonic = "DxRealQuality" + capability.capitalize(),
        progress_message = "Dx real quality " + capability + " %{label}",
    )

    return [OutputGroupInfo(dx_results = depset([out]))]

def _make_real_impl(capability, allowed_tools, output_suffix, has_rust_toolchain):
    """Makes one shard impl over the shared real pipeline action."""

    def _impl(target, ctx):
        return _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain)

    return _impl

def real_allowed_tools_error():
    """Validates aspect shards stay registry subsets."""
    allowed = (
        _CORE_LINT_TOOLS + _CORE_FORMAT_TOOLS + _CORE_TYPECHECK_TOOLS +
        _JS_LINT_TOOLS + _JS_FORMAT_TOOLS + _PY_LINT_TOOLS +
        _RUST_LINT_TOOLS + _RUST_FORMAT_TOOLS + _RUST_TYPECHECK_TOOLS +
        _JVM_LINT_TOOLS + _JVM_FORMAT_TOOLS
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
    "ruff": attr.label(
        default = "@dx_tools//:ruff",
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
_REAL_JS_LINT_ATTRS = _real_attrs_for(_JS_LINT_TOOLS)
_REAL_JS_FORMAT_ATTRS = _real_attrs_for(_JS_FORMAT_TOOLS)
_REAL_JVM_LINT_ATTRS = _real_attrs_for(_JVM_LINT_TOOLS)
_REAL_JVM_FORMAT_ATTRS = _real_attrs_for(_JVM_FORMAT_TOOLS)
_REAL_PY_LINT_ATTRS = _real_attrs_for(_PY_LINT_TOOLS)
_REAL_RUST_ATTRS = _REAL_BASE_ATTRS

_REAL_SHARDS = {
    "real_format": {"attrs": _REAL_CORE_ATTRS, "capability": "format", "doc": "Registers the exact-input real format pipeline action in dx_results.", "has_rust": False, "suffix": "", "tools": _CORE_FORMAT_TOOLS},
    "real_js_format": {"attrs": _REAL_JS_FORMAT_ATTRS, "capability": "format", "doc": "Additive JavaScript/JSON format family aspect (Prettier).", "has_rust": False, "suffix": "-js", "tools": _JS_FORMAT_TOOLS},
    "real_js_lint": {"attrs": _REAL_JS_LINT_ATTRS, "capability": "lint", "doc": "Additive JavaScript lint family aspect (ESLint opt-in).", "has_rust": False, "suffix": "-js", "tools": _JS_LINT_TOOLS},
    "real_jvm_format": {"attrs": _REAL_JVM_FORMAT_ATTRS, "capability": "format", "doc": "Additive JVM format family aspect (google-java-format/ktfmt).", "has_rust": False, "suffix": "-jvm", "tools": _JVM_FORMAT_TOOLS},
    "real_jvm_lint": {"attrs": _REAL_JVM_LINT_ATTRS, "capability": "lint", "doc": "Additive JVM lint family aspect (Checkstyle/Pmd/SpotBugs/ktlint; SpotBugs target-coupled via JavaInfo).", "has_rust": False, "suffix": "-jvm", "tools": _JVM_LINT_TOOLS},
    "real_lint": {"attrs": _REAL_CORE_ATTRS, "capability": "lint", "doc": "Registers the exact-input real lint pipeline action in dx_results.", "has_rust": False, "suffix": "", "tools": _CORE_LINT_TOOLS},
    "real_python_lint": {"attrs": _REAL_PY_LINT_ATTRS, "capability": "lint", "doc": "Additive Python lint family aspect (flake8/pylint/pydoclint).", "has_rust": False, "suffix": "-py", "tools": _PY_LINT_TOOLS},
    "real_rust_format": {"attrs": _REAL_RUST_ATTRS, "capability": "format", "doc": "Additive Rust format family aspect (toolchain rustfmt).", "has_rust": True, "suffix": "-rust", "tools": _RUST_FORMAT_TOOLS},
    "real_rust_lint": {"attrs": _REAL_RUST_ATTRS, "capability": "lint", "doc": "Additive Rust lint family aspect (delegated Clippy).", "has_rust": True, "suffix": "-rust", "tools": _RUST_LINT_TOOLS},
    "real_rust_typecheck": {"attrs": _REAL_RUST_ATTRS, "capability": "typecheck", "doc": "Additive Rust typecheck family aspect (delegated rustc).", "has_rust": True, "suffix": "-rust", "tools": _RUST_TYPECHECK_TOOLS},
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
    return aspect(
        implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
        attr_aspects = ["aspect_hints"],
        attrs = shard["attrs"],
    )

real_lint_aspect = _make_real_aspect("real_lint")

real_format_aspect = _make_real_aspect("real_format")

real_typecheck_aspect = _make_real_aspect("real_typecheck")

real_js_lint_aspect = _make_real_aspect("real_js_lint")

real_js_format_aspect = _make_real_aspect("real_js_format")

real_python_lint_aspect = _make_real_aspect("real_python_lint")

real_jvm_lint_aspect = _make_real_aspect("real_jvm_lint")

real_jvm_format_aspect = _make_real_aspect("real_jvm_format")

real_rust_lint_aspect = _make_real_aspect("real_rust_lint")

real_rust_format_aspect = _make_real_aspect("real_rust_format")

real_rust_typecheck_aspect = _make_real_aspect("real_rust_typecheck")
