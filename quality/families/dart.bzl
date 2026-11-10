"""Dart real quality aspects."""

load("//dart/private:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("//quality:dart_context.bzl", "DART_ANALYZE", "DART_FORMAT", "dart_external_root", "dart_merge_packages", "dart_package_entries", "dart_pubspec_stubs")
load("//quality:family_hooks.bzl", "stage")
load("//dart/private:dart_config.bzl", "DxDartConfigInfo")
load("//quality/private:native_config_impl.bzl", "bound_config_targets")
load("//quality:policy.bzl", "QualityPolicyInfo")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "dart": {
        "attr": attr.label(default = "//config:tool_dart", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "medium",
        "shard": "dart",
        "target_tool": True,
    },
}

SHARD_SPECS = {
    "real_dart_format": {
        "capability": "format",
        "doc": "Additive Dart format family aspect (dart format).",
        "shard": "dart",
        "suffix": "-dart",
    },
    "real_dart_lint": {
        "capability": "lint",
        "doc": "Additive Dart lint family aspect (dart analyze).",
        "shard": "dart",
        "suffix": "-dart",
    },
    "real_dart_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Dart typecheck family aspect (dart analyze).",
        "shard": "dart",
        "suffix": "-dart",
    },}

_DART_PACKAGE_CONFIG_REL = ".dart_tool/package_config.json"
_DART_OPTIONS_REL = "analysis_options.yaml"

def _dart_info(target, what):
    if DartAnalyzableInfo in target:
        return target[DartAnalyzableInfo].dart_info
    if DartInfo in target:
        return target[DartInfo]
    fail(what + ": dart needs a dart_library, dart_binary or dart_test target")

def _dart_context(target, ctx, what, capability, config_packages = []):
    """Writes the package config that gives each Dart file the language version Dart itself would use."""
    info = _dart_info(target, what)
    mode = DART_FORMAT if capability == "format" else DART_ANALYZE
    packages = info.transitive_packages.to_list()
    if mode == DART_ANALYZE:
        packages = dart_merge_packages(packages, config_packages)
    config = ctx.actions.declare_file(target.label.name + "-dart-package-config-" + capability + ".json")
    ctx.actions.write(output = config, content = json.encode({"configVersion": 2, "packages": dart_package_entries(packages, mode)}))
    options = ctx.actions.declare_file(target.label.name + "-dart-analysis-options-" + capability + ".yaml")
    ctx.actions.write(output = options, content = "")
    stubs = []
    if mode == DART_ANALYZE:
        for index, (path, content) in enumerate(dart_pubspec_stubs(packages)):
            stub = ctx.actions.declare_file("%s-dart-pubspec-%s-%d.yaml" % (target.label.name, capability, index))
            ctx.actions.write(output = stub, content = content)
            stubs.append((path, stub))
    return (config, options, stubs)

def _dart_hook(target, ctx, _resolved, capability, what, _shared):
    config_packages = []
    config_files = []
    if capability != "format":
        dart_config = bound_config_targets(ctx.rule.attr, ctx.attr._policy[QualityPolicyInfo].configs, what).get("dart")
        if dart_config and DxDartConfigInfo in dart_config:
            config_packages.extend(dart_config[DxDartConfigInfo].packages.to_list())
            config_files.extend(dart_config[DxDartConfigInfo].srcs.to_list())
            config_files.extend(dart_config[DxDartConfigInfo].resources.to_list())
    (package_config, dart_options, stubs) = _dart_context(target, ctx, what, capability, config_packages)

    pairs = []
    reserved = []
    if capability != "format":
        dart_info = _dart_info(target, what)
        dart_files = [f for f in dart_info.transitive_srcs.to_list() if f.extension == "dart"]
        dart_files += [f for f in dart_info.transitive_resources.to_list() if f.basename == "_embedder.yaml"]
        for f in dart_files + config_files:
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = dart_external_root(ws_path)
            if ".." in ws_path.split("/"):
                continue
            pairs.append((ws_path, f, False))
        for (stub_path, _) in stubs:
            reserved.append((stub_path, "generated pubspec.yaml stub collides with the staged source " + stub_path))

    def _with_config(hint, config_rel):
        # dart format reads `formatter:` from the nearest analysis_options.yaml, so the
        # hinted config (or an empty one) sits at the scratch root and nowhere else.
        options = dart_options
        tool_files = []
        inputs = []
        if hint != None:
            options = hint.config
        else:
            inputs.append(options)
        if config_rel != _DART_OPTIONS_REL:
            tool_files.append((_DART_OPTIONS_REL, options))
        stage(tool_files, inputs, _DART_PACKAGE_CONFIG_REL, package_config)
        for (stub_path, stub) in stubs:
            if stub_path == config_rel:
                fail(what + ": generated pubspec.yaml stub collides with the config " + stub_path)
            stage(tool_files, inputs, stub_path, stub)
        return struct(tool_files = tool_files, inputs = inputs)

    return real_context(resolve_pairs = pairs, reserved = reserved, with_config = _with_config)

HOOK_ORDER = ["dart"]

HOOKS = {"dart": _dart_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_dart_format_aspect = real_make_aspect(_CFG, "real_dart_format")

real_dart_lint_aspect = real_make_aspect(_CFG, "real_dart_lint")

real_dart_typecheck_aspect = real_make_aspect(_CFG, "real_dart_typecheck")

DartShardProbeInfo = provider(
    doc = "Shard names the dart shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_dart_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "dart", DartShardProbeInfo)

real_dart_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "dart", DartShardProbeInfo)

real_dart_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "dart", DartShardProbeInfo)
