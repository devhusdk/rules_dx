"""The real aspect engine: request building and shard aspects. Families reach it through hooks."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//libs/starlark:resources.bzl", "DX_QUALITY_TIERS", "dx_quality_resource_set")
load("//quality:adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load("//quality:execution_requirements.bzl", "dx_stage_execution_requirements")
load("//quality:native_config.bzl", "DxNativeConfigInfo", "bound_config_targets", "collect_native_configs", "missing_required_config_error", "native_config_scratch_rel")
load("//quality:parity_tests.bzl", "deferred_pipeline_error")
load("//quality:pipeline.bzl", "aspect_capability_blocked", "aspect_direct_maps", "aspect_family_selections", "filter_pipeline_by_tools", "ordered_pipeline_paths", "pipeline_inputs_for_paths", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline")
load("//quality:policy.bzl", "QualityPolicyInfo", "family_section_error")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo", "check_direct_sources", "upstream_source_class")
load("//quality:unavailable_tool.bzl", "DxUnavailableToolInfo")
load("//quality:support.bzl", "support_unpinned_tools", "support_unsupported_reasons")

_REAL_TOOL_KINDS = ["executable", "file", "setting", "toolchain", "upstream"]

_REAL_TOOLCHAIN_FILES = ["clippy_driver", "rustc", "rustfmt"]

# Where each wrapper-carried tool comes from: (wrappers, defs.bzl, tool label).
_TARGET_TOOL_OWNERS = {
    "dart": ("dart_library, dart_binary or dart_test", "@rules_dx//dart/rules:defs.bzl", "@rules_dx//dart/tools:dart"),
    "dfmt": ("d_library, d_binary or d_test", "@rules_dx//d/rules:defs.bzl", "@rules_dx//d/tools:dfmt"),
    "dscanner": ("d_library, d_binary or d_test", "@rules_dx//d/rules:defs.bzl", "@rules_dx//d/tools:dscanner"),
    "swift_format": ("swift_library, swift_binary or swift_test", "@rules_dx//swift/rules:defs.bzl", "@rules_dx//swift/tools:swift_format"),
    "swift_lint": ("swift_library, swift_binary or swift_test", "@rules_dx//swift/rules:defs.bzl", "@rules_dx//swift/tools:swift_format"),
    "zig_ast_check": ("zig_library, zig_binary or zig_test", "@rules_dx//zig/rules:defs.bzl", "@rules_dx//zig/tools:zig"),
    "zig_fmt": ("zig_library, zig_binary or zig_test", "@rules_dx//zig/rules:defs.bzl", "@rules_dx//zig/tools:zig"),
    "perlcritic": ("perl_library, perl_binary or perl_test", "@rules_dx//perl/rules:defs.bzl", "@rules_dx//perl/tools:perlcritic"),
    "perltidy": ("perl_library, perl_binary or perl_test", "@rules_dx//perl/rules:defs.bzl", "@rules_dx//perl/tools:perltidy"),
    "julia_format": ("julia_library, julia_binary or julia_test", "@rules_dx//julia/rules:defs.bzl", "@rules_dx//julia/tools:julia_format"),
}

def _tool_files_to_run(target, flag, tool, what):
    """Returns the flag's tool, else the one the target carries."""
    ftr = flag[DefaultInfo].files_to_run
    if ftr != None and ftr.executable != None:
        return ftr
    if QualityToolsInfo in target and tool in target[QualityToolsInfo].tools:
        return target[QualityToolsInfo].tools[tool]
    wrappers, defs, tool_label = _TARGET_TOOL_OWNERS[tool]
    fail(what + ": " + tool + " has no executable; use " + wrappers + " from " + defs + ", or set --@rules_dx//config:tool_" + tool + "=" + tool_label)

def _unique(items):
    """Returns items without repeats, in first-seen order."""
    return list({item: True for item in items}.keys())

def real_unavailable_tools(tools, stage_tools, delegated, pinned_counts, marked = {}):
    """Returns the stage tools with no usable artifact here: empty optional pins and marked stubs."""
    return sorted([
        tool
        for tool in _unique(stage_tools)
        if tool not in delegated and (marked.get(tool, False) or (tools[tool]["kind"] == "file" and "fallback" in tools[tool] and pinned_counts.get(tool, 1) == 0))
    ])

def real_skip_notes(tools, reasons):
    """Returns one skip note per unavailable tool, naming the unsupported reason when known."""
    notes = []
    for tool in tools:
        note = tool + ": skipped, no pinned artifact for this execution platform"
        if tool in reasons:
            note += " (" + reasons[tool] + ")"
        notes.append(note)
    return notes

def _real_tool_launch(cfg, target, ctx, stage_tools, delegated, has_toolchain, what):
    """Resolves each launched tool's binary, extra inputs, run tools and env from the tool table."""
    binaries = {}
    extra = {}
    run_tools = []
    env = {}
    for tool in _unique(stage_tools):
        spec = cfg.tools[tool]
        kind = spec["kind"]
        if kind == "upstream" or tool in delegated:
            continue
        if kind == "toolchain":
            if has_toolchain:
                binaries[tool] = cfg.toolchain.file(ctx, spec["toolchain_file"])
            continue
        if kind == "file" and "fallback" in spec:
            # An optional pin: the hub resolves an empty file set on platforms without an artifact.
            files = getattr(ctx.files, "_" + tool)
            if len(files) != 1:
                # Unreachable for stages real_unavailable_tools already dropped.
                fail(what + ": " + tool + " has no pinned artifact for this execution platform; set --@rules_dx//config:tool_" + tool + spec["fallback"])
            binaries[tool] = files[0]
        elif kind == "file":
            binaries[tool] = getattr(ctx.file, "_" + tool)
            runtime = getattr(ctx.attr, "_" + tool)[DefaultInfo].default_runfiles.files.to_list()
            if runtime:
                extra[tool] = runtime
        elif kind == "executable":
            tool_target = getattr(ctx.attr, "_" + tool)
            if DxUnavailableToolInfo in tool_target:
                # Unreachable for stages real_unavailable_tools already dropped.
                fail(what + ": " + tool + " is unavailable on this execution platform")
            binaries[tool] = getattr(ctx.executable, "_" + tool)
            if spec.get("extra") == "files":
                extra[tool] = tool_target[DefaultInfo].files.to_list()
            elif spec.get("extra") == "runfiles":
                extra[tool] = tool_target[DefaultInfo].default_runfiles.files.to_list()
            if spec.get("run_tool", True):
                run_tools.append(tool_target[DefaultInfo].files_to_run)
        elif spec.get("target_tool", False):
            ftr = _tool_files_to_run(target, getattr(ctx.attr, "_" + tool), tool, what)
            binaries[tool] = ftr.executable
            run_tools.append(ftr)
        else:
            ftr = getattr(ctx.attr, "_" + tool)[DefaultInfo].files_to_run
            if ftr == None or ftr.executable == None:
                fail(what + ": " + tool + " has no executable; set --@rules_dx//config:tool_" + tool + spec["fallback"])
            binaries[tool] = ftr.executable
            run_tools.append(ftr)
        pairs = [real_request_tool_env(pair[0], pair[1]) for pair in spec.get("env", [])]
        if spec.get("runfiles_env", False):
            pairs.append(real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles"))
        if len(pairs) > 0:
            env[tool] = pairs
    return struct(binaries = binaries, extra = extra, run_tools = run_tools, env = env)

def real_tool_table_shape_error(table):
    """Returns the first broken shape rule in a real tool table, else an empty string."""
    for tool in sorted(table.keys()):
        spec = table[tool]
        prefix = "real aspects: table tool '" + tool + "' "
        if tool not in REAL_ADAPTERS:
            return prefix + "is outside REAL_ADAPTERS"
        if spec.get("resources") not in DX_QUALITY_TIERS:
            return prefix + "names unknown resources tier '" + str(spec.get("resources")) + "'"
        for capability in spec["capabilities"]:
            if capability not in REAL_ADAPTERS[tool]:
                return prefix + "names unwired capability '" + capability + "'"
        kind = spec.get("kind")
        if kind not in _REAL_TOOL_KINDS:
            return prefix + "names unknown kind '" + str(kind) + "'"
        if kind in ["executable", "file", "setting"] and "attr" not in spec:
            return prefix + "needs an attr for kind '" + kind + "'"
        if kind not in ["executable", "file", "setting"] and "attr" in spec:
            return prefix + "must not set an attr for kind '" + kind + "'"
        if kind == "setting" and ("fallback" in spec) == ("target_tool" in spec):
            return prefix + "needs exactly one of fallback or target_tool"
        if kind not in ["setting", "file"] and "fallback" in spec:
            return prefix + "sets fallback outside the setting and file kinds"
        if kind != "setting" and "target_tool" in spec:
            return prefix + "sets target_tool outside the setting kind"
        if spec.get("target_tool", False) and tool not in _TARGET_TOOL_OWNERS:
            return prefix + "has no wrapper owner in _TARGET_TOOL_OWNERS"
        if kind != "executable" and ("extra" in spec or "run_tool" in spec or "runfiles_env" in spec):
            return prefix + "sets extra, run_tool or runfiles_env outside the executable kind"
        if spec.get("extra", "files") not in ["files", "runfiles"]:
            return prefix + "names unknown extra '" + str(spec.get("extra")) + "'"
        if (kind == "toolchain") != ("toolchain_file" in spec) or spec.get("toolchain_file", "rustc") not in _REAL_TOOLCHAIN_FILES:
            return prefix + "needs a known toolchain_file exactly when its kind is toolchain"
        for pair in spec.get("env", []):
            if type(pair) != "list" or len(pair) != 2 or type(pair[0]) != "string" or type(pair[1]) != "string":
                return prefix + "has an env entry that is not a pair of strings"
    return ""

_REAL_BASE_ATTRS = {
    "_policy": attr.label(
        default = "//config:workspace",
        providers = [QualityPolicyInfo],
    ),
    "_online": attr.label(default = "//config:lint_online"),
    "_breaking_against": attr.label(default = "//config:buf_breaking_against"),
    "_runner": attr.label(
        default = "//quality/runner:quality_runner",
        executable = True,
        cfg = "exec",
        allow_files = True,
    ),
}

def _real_attrs_for(table, tools):
    """Builds one shard attr set from the attrs in the tool table."""
    return _REAL_BASE_ATTRS | {"_" + tool: table[tool]["attr"] for tool in tools if "attr" in table[tool]}

def real_context(
        resolved = None,
        early_inputs = [],
        resolve_pairs = [],
        reserved = [],
        tool_files = [],
        inputs = [],
        late_tool_files = [],
        late_inputs = [],
        upstream = [],
        with_config = None,
        env = [],
        run_tools = [],
        options = {},
        delegated = False):
    """Builds what one tool's context hook hands the core.

    resolved is the pruned pipeline, or None to keep it. early_inputs are staged before the
    resolve files. resolve_pairs are (workspace path, File, strict) entries, deduplicated
    against the sources and siblings. reserved are (workspace path, error) pairs that must
    stay unstaged. tool_files are (relative path, File) pairs, upstream the Files whose results the tool reads,
    and inputs the Files the action needs besides them; the late_ lists come after the native config. with_config is called
    with the tool's native config hint (or None) and its relative path, and returns a struct of
    tool_files and inputs. options may hold edition, dialect, args, search_paths and path_tools.
    """
    return struct(
        resolved = resolved,
        early_inputs = early_inputs,
        resolve_pairs = resolve_pairs,
        reserved = reserved,
        tool_files = tool_files,
        inputs = inputs,
        late_tool_files = late_tool_files,
        late_inputs = late_inputs,
        upstream = upstream,
        with_config = with_config,
        env = env,
        run_tools = run_tools,
        options = options,
        delegated = delegated,
    )

def _shard_tools(tools, shard, capability):
    """Lists wired tools for one shard and capability, or for every capability when capability is None."""
    return sorted([tool for tool in tools if tools[tool]["shard"] == shard and (capability == None or capability in tools[tool]["capabilities"])])

def _expand_shard(tools_table, spec):
    """Expands one shard spec into its tools, attrs and toolchain flag."""
    tools = _shard_tools(tools_table, spec["shard"], spec["capability"])
    attr_tools = _shard_tools(tools_table, spec["shard"], None) if spec.get("shared_attrs", False) else tools
    return spec | {
        "attrs": _real_attrs_for(tools_table, attr_tools) | spec.get("extra_attrs", {}),
        "has_toolchain": len([tool for tool in tools if tools_table[tool]["kind"] == "toolchain"]) > 0,
        "tools": tools,
    }

def real_config(tools, shard_specs, hook_order, context_hooks, source_recognizers, harness_sources, toolchain, check_policy = True):
    """Bundles the tool table and the family hooks the engine runs.

    context_hooks maps a tool name to f(target, ctx, resolved, capability, what, shared) that
    returns real_context(...). The core calls them in hook_order only, for the tools a target
    still resolves to. source_recognizers are f(target, ctx) returning None or
    struct(files, error). harness_sources is f(target, capability, direct_sources) returning
    the direct sources with any test-harness sources added. toolchain is
    struct(type, file) where file(ctx, name) returns one toolchain file.
    A tool spec without capabilities runs every capability tools.bzl lists for it.
    check_policy is False for a family file whose table lacks the other families' tools; the
    combined config validates the workspace policy.
    """
    tools = {
        tool: spec if "capabilities" in spec else spec | {"capabilities": sorted(REAL_ADAPTERS[tool].keys())}
        for tool, spec in tools.items()
    }
    return struct(
        tools = tools,
        shards = {name: _expand_shard(tools, spec) for name, spec in shard_specs.items()},
        wired = {
            capability: sorted([tool for tool in tools if capability in tools[tool]["capabilities"]])
            for capability in ["format", "lint", "typecheck"]
        },
        hook_order = hook_order,
        context_hooks = context_hooks,
        source_recognizers = source_recognizers,
        harness_sources = harness_sources,
        toolchain = toolchain,
        check_policy = check_policy,
    )

def real_wired_tools(cfg):
    """Maps each capability to the sorted tool names the table wires for it."""
    return {capability: list(tools) for capability, tools in cfg.wired.items()}

def real_policy_error(cfg, policy, capability, what):
    """Validates the selected workspace policy for one real capability."""
    if not cfg.check_policy:
        return ""
    unpinned = support_unpinned_tools(capability)
    for family_id in sorted(policy.families.keys()):
        section = policy.families[family_id]
        error = family_section_error(
            family_id,
            capability,
            getattr(section, capability),
            section.disabled,
            cfg.wired[capability],
            unpinned,
        )
        if error != "":
            return what + ": " + error
    return ""

def real_upstream_direct_sources(recognizers, target, ctx):
    """Derives directly owned sources for an ordinary upstream native target."""
    recognized = False
    files = []
    for recognize in recognizers:
        found = recognize(target, ctx)
        if found == None:
            continue
        recognized = True
        if found.error != "":
            return (True, {}, found.error)
        files.extend(found.files)
    if not recognized:
        return (False, {}, "")
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

def real_resolved_stages(cfg, target, ctx, capability, what):
    """Resolves one target's pipeline stages for both the real shards and the probe."""
    policy = ctx.attr._policy[QualityPolicyInfo]
    policy_error = real_policy_error(cfg, policy, capability, what)
    if policy_error != "":
        fail(policy_error)
    if aspect_capability_blocked(ctx.rule.attr, capability):
        return ([], None, None)
    if target.label.repo_name != "":
        return ([], None, None)
    if QualitySourcesInfo in target:
        direct_sources = target[QualitySourcesInfo].direct_sources
    else:
        # The dx wrapper's native "<name>_upstream" target shares its sources with the
        # forwarder, which is the sole origin; skip it to avoid duplicate findings.
        if target.label.name.endswith("_upstream"):
            return ([], None, None)
        (recognized, derived, ownership_error) = real_upstream_direct_sources(cfg.source_recognizers, target, ctx)
        if ownership_error != "":
            fail(what + ": " + ownership_error)
        if not recognized:
            return ([], None, None)
        direct_sources = derived
    direct_sources = cfg.harness_sources(target, capability, direct_sources)
    if len(direct_sources) == 0:
        return ([], None, None)

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
    return (resolved, direct_files, path_to_file)

def _staged(known, ws_path):
    """Returns the File already staged at ws_path in any of the maps, else None."""
    for files in known:
        if ws_path in files:
            return files[ws_path]
    return None

def _real_pipeline_action(cfg, target, ctx, capability, allowed_tools, output_suffix, has_toolchain, shard_name):
    what = "real_aspect (" + str(target.label) + ")"
    (resolved, direct_files, path_to_file) = real_resolved_stages(cfg, target, ctx, capability, what)
    if len(resolved) == 0:
        return []
    resolved = filter_pipeline_by_tools(resolved, allowed_tools)
    if len(resolved) == 0:
        return []

    shared = struct(path_to_file = path_to_file, direct_files = direct_files, has_toolchain = has_toolchain)
    contexts = {}
    for tool in cfg.hook_order:
        if tool not in [stage["tool"] for stage in resolved]:
            continue
        found = cfg.context_hooks[tool](target, ctx, resolved, capability, what, shared)
        if found.resolved != None:
            resolved = found.resolved
            if len(resolved) == 0:
                return []
        contexts[tool] = found

    stage_tools = [stage["tool"] for stage in resolved]
    delegated = {tool: True for tool, found in contexts.items() if found.delegated}
    pinned_counts = {tool: len(getattr(ctx.files, "_" + tool)) for tool in stage_tools if cfg.tools[tool]["kind"] == "file" and "fallback" in cfg.tools[tool]}
    marked = {tool: True for tool in stage_tools if cfg.tools[tool]["kind"] == "executable" and DxUnavailableToolInfo in getattr(ctx.attr, "_" + tool)}
    target_unavailable = getattr(target[QualityToolsInfo], "unavailable", []) if QualityToolsInfo in target else []
    for tool in stage_tools:
        if cfg.tools[tool].get("target_tool", False) and tool in target_unavailable and getattr(ctx.attr, "_" + tool)[DefaultInfo].files_to_run.executable == None:
            marked[tool] = True
    unavailable = real_unavailable_tools(cfg.tools, stage_tools, delegated, pinned_counts, marked)
    skip_groups = {}
    if len(unavailable) > 0:
        resolved = [stage for stage in resolved if stage["tool"] not in unavailable]
        notes = real_skip_notes(unavailable, support_unsupported_reasons(unavailable))
        note_file = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + "-skips.txt")
        ctx.actions.write(note_file, "\n".join(notes) + "\n")
        skip_groups = {"dx_skips": depset([note_file])}
        if len(resolved) == 0:
            return [OutputGroupInfo(**skip_groups)]
        stage_tools = [stage["tool"] for stage in resolved]
    contexts = {tool: found for tool, found in contexts.items() if tool in stage_tools}

    bound_targets = bound_config_targets(ctx.rule.attr, ctx.attr._policy[QualityPolicyInfo].configs, str(target.label))
    configs_by_tool = collect_native_configs([], stage_tools, str(target.label), {tool: t[DxNativeConfigInfo] for tool, t in bound_targets.items()})

    for stage in resolved:
        config_error = missing_required_config_error(stage["tool"], capability, configs_by_tool, str(target.label))
        if config_error != "":
            fail(config_error)

    launch = _real_tool_launch(cfg, target, ctx, stage_tools, delegated, has_toolchain, what)
    tool_binaries = launch.binaries
    tool_extra = launch.extra

    out = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + ".pb")

    ordered_paths = ordered_pipeline_paths(resolved)
    inputs = pipeline_inputs_for_paths(ordered_paths, path_to_file)

    sibling_pairs = {}
    if ("markdown_check" in stage_tools or "lychee" in stage_tools) and hasattr(ctx.rule.attr, "markdown_siblings"):
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
    tool_env_entries = launch.env
    run_tools = launch.run_tools
    for tool in cfg.hook_order:
        if tool not in contexts:
            continue
        found = contexts[tool]
        inputs.extend(found.early_inputs)
        for (ws_path, f, strict) in found.resolve_pairs:
            staged_as = _staged([path_to_file, sibling_pairs, resolve_pairs], ws_path)
            if staged_as != None:
                if strict and staged_as != f:
                    fail(what + ": " + tool + " would stage two different files at " + ws_path + ": " + staged_as.path + " and " + f.path)
                continue
            resolve_pairs[ws_path] = f
        for (ws_path, error) in found.reserved:
            if _staged([path_to_file, sibling_pairs, resolve_pairs], ws_path) != None:
                fail(what + ": " + error)
        if len(found.env) > 0:
            tool_env_entries.setdefault(tool, []).extend(found.env)
        run_tools = run_tools + found.run_tools
    requirements = dict(dx_stage_execution_requirements(stage_tools, capability))
    if "lychee" in stage_tools and hasattr(ctx.attr, "_online") and ctx.attr._online[BuildSettingInfo].value:
        tool_env_entries.setdefault("lychee", []).append(["DX_LINT_ONLINE", "1"])
        requirements["requires-network"] = "1"
    if "buf" in stage_tools and capability == "lint" and hasattr(ctx.attr, "_breaking_against"):
        image = ctx.attr._breaking_against[BuildSettingInfo].value
        if image:
            # The base image lives outside the workspace, so the action reads it unsandboxed and uncached.
            tool_env_entries.setdefault("buf", []).append(["DX_BUF_BREAKING_AGAINST", image])
            requirements["no-sandbox"] = "1"
            requirements["no-cache"] = "1"
            requirements["no-remote"] = "1"
    request_resolves = []
    for ws_path in sorted(resolve_pairs.keys()):
        f = resolve_pairs[ws_path]
        request_resolves.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    request_tools = {}
    for tool in stage_tools:
        found = contexts.get(tool)
        binary_path = None
        if tool not in delegated and cfg.tools[tool]["kind"] != "upstream":
            binary = tool_binaries[tool]
            binary_path = binary.path
            inputs.append(binary)
            inputs.extend(tool_extra.get(tool, []))
        tool_files = []
        options = {}
        if found != None:
            options = found.options
            tool_files.extend(found.tool_files)
            inputs.extend(found.inputs)
        config_rel = None
        if tool in configs_by_tool:
            hint = configs_by_tool[tool]
            config_rel = native_config_scratch_rel(hint.config.short_path)
            if config_rel == None:
                fail("native_config (%s): config %s for %s escapes the scratch" % (tool, hint.config.short_path, what))
            for f in sorted(hint.closure.to_list(), key = lambda f: f.short_path):
                closure_rel = native_config_scratch_rel(f.short_path)
                if closure_rel == None:
                    fail("native_config (%s): file %s for %s escapes the scratch" % (tool, f.short_path, what))
                tool_files.append((closure_rel, f))
                inputs.append(f)
        if found != None:
            if found.with_config != None:
                late = found.with_config(configs_by_tool.get(tool), config_rel)
                tool_files.extend(late.tool_files)
                inputs.extend(late.inputs)
            tool_files.extend(found.late_tool_files)
            inputs.extend(found.late_inputs)
        request_tools[tool] = real_request_tool(
            binary_path = binary_path,
            config_rel = config_rel,
            edition = options.get("edition"),
            dialect = options.get("dialect"),
            args = options.get("args", []),
            files = [real_request_tool_file(rel, f.path) for (rel, f) in tool_files],
            env = tool_env_entries.get(tool, []),
            upstream = [f.path for f in (found.upstream if found != None else [])],
            search_paths = [f.path for f in options.get("search_paths", [])],
            path_tools = [{"binary": f.path, "name": name} for (name, f) in options.get("path_tools", [])],
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
    args.add("--shard", shard_name)

    ctx.actions.run(
        executable = ctx.executable._runner,
        inputs = depset(inputs),
        tools = run_tools,
        outputs = [out],
        arguments = [args],
        execution_requirements = requirements,
        mnemonic = "DxRealQuality" + capability.capitalize(),
        resource_set = real_quality_resource_set(cfg, sorted(tool_binaries.keys())),
        progress_message = "Dx real quality " + capability + " (" + shard_name + ") %{label}",
        toolchain = cfg.toolchain.type if has_toolchain else None,
    )

    return [OutputGroupInfo(dx_results = depset([out]), **skip_groups)]

def real_quality_resource_set(cfg, tools):
    """Returns the resource_set callback for the costliest tier among the launched tools."""
    return dx_quality_resource_set([cfg.tools[tool]["resources"] for tool in tools])

def real_tool_table_error(cfg):
    """Validates the real tool table and that a built shard runs every tool capability."""
    error = real_tool_table_shape_error(cfg.tools)
    if error != "":
        return error
    wired = real_wired_capabilities(cfg)
    for tool in sorted(cfg.tools.keys()):
        if wired.get(tool, []) != sorted(cfg.tools[tool]["capabilities"]):
            return "real aspects: table tool '" + tool + "' has a capability no shard runs"
    return ""

def _workspace_crate(ctx):
    """Returns the in-workspace crate a test rule points at, or None."""
    crate = getattr(ctx.rule.attr, "crate", None)
    if type(crate) != "Target" or crate.label.repo_name != "" or OutputGroupInfo not in crate:
        return None
    return crate

def real_make_aspect(cfg, shard_name):
    """Builds one shard aspect from the shard table."""
    shard = cfg.shards[shard_name]

    follows_crate = shard["capability"] == "format"

    def _impl(target, ctx):
        found = _real_pipeline_action(cfg, target, ctx, shard["capability"], shard["tools"], shard["suffix"], shard["has_toolchain"], shard_name)
        if not follows_crate:
            return found
        crate = _workspace_crate(ctx)
        if crate == None or "dx_results" not in crate[OutputGroupInfo]:
            return found
        own = found[0]["dx_results"] if len(found) > 0 and "dx_results" in found[0] else depset()
        return [OutputGroupInfo(dx_results = depset(transitive = [own, crate[OutputGroupInfo]["dx_results"]]))]

    return aspect(
        implementation = _impl,
        doc = shard["doc"],
        attr_aspects = ["aspect_hints", "crate"] if follows_crate else ["aspect_hints"],
        attrs = shard["attrs"],
        toolchains = shard.get("toolchains", []),
        requires = shard.get("requires", []),
        required_aspect_providers = shard.get("required_aspect_providers", []),
    )

RealShardProbeInfo = provider(
    doc = "Shard names the shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

def real_shard_result_stems(cfg):
    """Maps each shard's dx_results stem ("real-" + capability + suffix) to its shard name."""
    return {
        "real-" + shard["capability"] + shard["suffix"]: shard_name
        for shard_name, shard in cfg.shards.items()
    }

def real_wired_capabilities(cfg):
    """Maps each tool to the sorted capabilities a built shard aspect runs for it."""
    wired = {}
    for shard in cfg.shards.values():
        for tool in shard["tools"]:
            if shard["capability"] not in wired.setdefault(tool, []):
                wired[tool].append(shard["capability"])
    return {tool: sorted(caps) for tool, caps in wired.items()}

def _probe_shards(cfg, resolved, capability):
    """Names every shard of the capability whose tool filter keeps a stage."""
    shards = []
    for shard_name in sorted(cfg.shards.keys()):
        shard = cfg.shards[shard_name]
        if shard["capability"] != capability:
            continue
        if len(filter_pipeline_by_tools(resolved, shard["tools"])) > 0:
            shards.append(shard_name)
    return shards

def real_make_probe_aspect(cfg, capability, family = "", info = RealShardProbeInfo):
    """Builds the tool-free shard probe aspect for one capability.

    A family probe passes its name and the provider its file declares, so two
    probes on one target neither share an output file nor a provider.
    """

    def _impl(target, ctx):
        what = "real_aspect (" + str(target.label) + ")"
        (resolved, _direct_files, _path_to_file) = real_resolved_stages(cfg, target, ctx, capability, what)
        crate_shards = []
        if capability == "format":
            crate = getattr(ctx.rule.attr, "crate", None)
            if type(crate) == "Target" and crate.label.repo_name == "" and info in crate:
                crate_shards = list(crate[info].shards)
        shards = _probe_shards(cfg, resolved, capability) if len(resolved) > 0 else []
        shards = sorted({name: True for name in shards + crate_shards}.keys())
        if len(shards) == 0:
            return []
        out = ctx.actions.declare_file(target.label.name + ".dx-shards-" + capability + ("-" + family if family else "") + ".json")
        ctx.actions.write(out, json.encode({"shards": shards}))
        return [
            OutputGroupInfo(dx_shards = depset([out])),
            info(capability = capability, shards = shards),
        ]

    return aspect(
        implementation = _impl,
        attr_aspects = ["aspect_hints", "crate"] if capability == "format" else ["aspect_hints"],
        attrs = {"_policy": _REAL_BASE_ATTRS["_policy"]},
        doc = "Writes the " + capability + " shard names each target needs to the dx_shards output group.",
    )
