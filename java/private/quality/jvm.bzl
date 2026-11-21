"""JVM real quality aspects."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//core/quality:family_hooks.bzl", "default_files_hook", "stage", "without")
load("//java/private/quality:jvm_check.bzl", "JvmCheckInfo", "android_sdk_jars", "jvm_android_sdk_aspect", "jvm_check_aspect")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//java/private/quality/tools:jar_lsp_tool.bzl", "PklLspToolInfo")

TOOLS = {
    "detekt": {
        "attr": attr.label(default = "//java/private/quality/tools:detekt", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "google_java_format": {
        "attr": attr.label(default = "//java/private/quality/tools:google_java_format", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "javac": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "kotlinc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "ktfmt": {
        "attr": attr.label(default = "//java/private/quality/tools:ktfmt", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "pmd": {
        "attr": attr.label(default = "//java/private/quality/tools:pmd", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "pkl_lsp": {
        "attr": attr.label(default = "//java/private/quality/tools:pkl_lsp", cfg = "exec", executable = True),
        "extra": "runfiles",
        "kind": "executable",
        "resources": "heavy",
        "shard": "pkl",
    },
    "spotbugs": {
        "attr": attr.label(default = "//java/private/quality/tools:spotbugs", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
}

_SPOTBUGS_EFFORT = {"standard": "default", "strict": "max"}

SHARD_SPECS = {
    "real_jvm_format": {
        "capability": "format",
        "doc": "Additive JVM format family aspect (google-java-format/ktfmt).",
        "shard": "jvm",
    },
    "real_jvm_lint": {
        "capability": "lint",
        "doc": "Additive JVM lint family aspect (Pmd/SpotBugs; SpotBugs target-coupled via JavaInfo).",
        "extra_attrs": {
            "_jacoco_runtime": attr.label(
                default = "@bazel_tools//tools/jdk:JacocoCoverage",
                providers = [JavaInfo],
                doc = "JaCoCo runtime SpotBugs needs to resolve coverage-instrumented classes.",
            ),
            "_config_tier": attr.label(default = "//config:config_tier"),
        },
        "requires": [jvm_android_sdk_aspect],
        "shard": "jvm",
    },
    "real_jvm_typecheck": {
        "capability": "typecheck",
        "doc": "Additive JVM typecheck family aspect (delegated javac over rules_java targets and kotlinc over rules_kotlin targets).",
        "requires": [jvm_check_aspect],
        "shard": "jvm",
    },
    "real_pkl_lint": {
        "capability": "lint",
        "doc": "Additive Pkl lint family aspect (pkl-lsp diagnostics through lsp_check).",
        "shard": "pkl",
    },
}

def _jvm_jar_key(jar):
    """Returns the jar path without its header or ijar marker."""
    name = jar.basename.removeprefix("header_")
    for suffix in ["-hjar.jar", "-ijar.jar"]:
        if name.endswith(suffix):
            name = name.removesuffix(suffix) + ".jar"
            break
    return "jar:" + jar.short_path.rpartition("/")[0] + "/" + name

def _jacoco_runtime_jars(ctx):
    """Returns the jacoco agent runtime jars (a separate target, not a tool classpath)."""
    return ctx.attr._jacoco_runtime[JavaInfo].transitive_runtime_jars.to_list()

def jvm_aux_jars(java_info, owned):
    """Returns the runtime classpath jars plus compile-only jars not in owned."""
    runtime = [java_info.transitive_runtime_jars]
    compile = [java_info.transitive_compile_time_jars]
    if java_info.compilation_info != None:
        runtime.append(java_info.compilation_info.runtime_classpath)
        compile.append(java_info.compilation_info.compilation_classpath)
    auxiliary = {}
    seen = {}
    for jar in owned.values():
        seen[_jvm_jar_key(jar)] = True
        if jar.owner != None:
            seen[str(jar.owner)] = True
    for jar in depset(transitive = runtime).to_list():
        if jar.path not in owned:
            auxiliary[jar.path] = jar
            seen[_jvm_jar_key(jar)] = True
            if jar.owner != None:
                seen[str(jar.owner)] = True
    for jar in depset(transitive = compile).to_list():
        if jar.path in owned or jar.path in auxiliary or _jvm_jar_key(jar) in seen:
            continue
        if jar.owner != None and str(jar.owner) in seen:
            continue
        auxiliary[jar.path] = jar
    return auxiliary

def _spotbugs_hook(target, ctx, resolved, _capability, _what, _shared):
    if JavaInfo not in target:
        return without(resolved, "spotbugs")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None and ".." not in output.class_jar.short_path.split("/"):
            owned[output.class_jar.path] = output.class_jar
    auxiliary = jvm_aux_jars(target[JavaInfo], owned)
    for jar in android_sdk_jars(target).to_list():
        if jar.path not in owned:
            auxiliary[jar.path] = jar
    if ctx.configuration.coverage_enabled and hasattr(ctx.attr, "_jacoco_runtime"):
        for jar in _jacoco_runtime_jars(ctx):
            if jar.path not in owned:
                auxiliary[jar.path] = jar
    if len(owned) == 0:
        return without(resolved, "spotbugs")
    tool_files = []
    inputs = []
    for jar in sorted(owned.values(), key = lambda f: f.short_path):
        stage(tool_files, inputs, "dx-spotbugs/owned/" + jar.short_path, jar)
    for index, jar in enumerate(sorted(auxiliary.values(), key = lambda f: f.short_path)):
        stage(tool_files, inputs, "dx-spotbugs/aux/%d-%s" % (index, jar.basename), jar)
    effort = _SPOTBUGS_EFFORT[ctx.attr._config_tier[BuildSettingInfo].value]
    options = {"args": [effort]}
    return real_context(tool_files = tool_files, inputs = inputs, options = options)

def jvm_check_hook(tool):
    """Returns the context hook that hands one JvmCheckInfo tool's log to the runner."""

    def _hook(target, _ctx, resolved, _capability, _what, _shared):
        if JvmCheckInfo not in target or target[JvmCheckInfo].tool != tool:
            return without(resolved, tool)
        files = [target[JvmCheckInfo].log] + target[JvmCheckInfo].unused_deps
        return real_context(upstream = files, late_inputs = files)

    return _hook

_PKL_FILES_HOOK = default_files_hook(["pkl"])

def _pkl_lsp_hook(target, ctx, resolved, capability, what, shared):
    context = _PKL_FILES_HOOK(target, ctx, resolved, capability, what, shared)
    bundle = ctx.attr._pkl_lsp[PklLspToolInfo]
    return real_context(
        resolve_pairs = context.resolve_pairs,
        options = {"path_tools": [("java", bundle.java), ("pkl-lsp.jar", bundle.jar), ("pkl-lsp-natives.jar", bundle.natives)]},
    )

HOOK_ORDER = ["spotbugs", "javac", "kotlinc", "pkl_lsp"]

HOOKS = {
    "spotbugs": _spotbugs_hook,
    "javac": jvm_check_hook("javac"),
    "kotlinc": jvm_check_hook("kotlinc"),
    "pkl_lsp": _pkl_lsp_hook,
}

_JVM_SOURCE_KINDS = ["android_library", "java_library", "kt_android_library", "kt_jvm_library"]

def _jvm_sources(target, ctx):
    if JavaInfo not in target or ctx.rule.kind not in _JVM_SOURCE_KINDS:
        return None
    srcs = getattr(ctx.rule.files, "srcs", None)
    if srcs == None:
        return struct(files = [], error = "upstream " + ctx.rule.kind + " target has no readable srcs; unsupported ownership is unchecked, not clean")
    return struct(files = list(srcs), error = "")

RECOGNIZER = _jvm_sources

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [RECOGNIZER],
)

real_jvm_format_aspect = real_make_aspect(_CFG, "real_jvm_format")

real_jvm_lint_aspect = real_make_aspect(_CFG, "real_jvm_lint")

real_jvm_typecheck_aspect = real_make_aspect(_CFG, "real_jvm_typecheck")

real_pkl_lint_aspect = real_make_aspect(_CFG, "real_pkl_lint")

JvmShardProbeInfo = provider(
    doc = "Shard names the jvm shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_jvm_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "jvm", JvmShardProbeInfo)

real_jvm_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "jvm", JvmShardProbeInfo)

real_jvm_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "jvm", JvmShardProbeInfo)
