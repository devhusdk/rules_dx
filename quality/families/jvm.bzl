"""JVM and Scala real quality aspects."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:jvm_check.bzl", "JvmCheckInfo", "jvm_check_aspect")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "checkstyle": {
        "attr": attr.label(default = "//quality/tools/jvm:checkstyle", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "detekt": {
        "attr": attr.label(default = "//quality/tools/jvm:detekt", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "google_java_format": {
        "attr": attr.label(default = "//quality/tools/jvm:google_java_format", cfg = "exec", executable = True),
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
        "attr": attr.label(default = "//quality/tools/jvm:ktfmt", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "ktlint": {
        "attr": attr.label(default = "//quality/tools/jvm:ktlint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "pmd": {
        "attr": attr.label(default = "//quality/tools/jvm:pmd", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "scalac": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "scalafix": {
        "attr": attr.label(default = "//quality/tools/jvm:scalafix", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "scala",
    },
    "scalafmt": {
        "attr": attr.label(default = "@dx_tools//:scalafmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "heavy",
        "shard": "scala",
    },
    "spotbugs": {
        "attr": attr.label(default = "//quality/tools/jvm:spotbugs", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
}

SHARD_SPECS = {
    "real_jvm_format": {
        "capability": "format",
        "doc": "Additive JVM format family aspect (google-java-format/ktfmt).",
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_jvm_lint": {
        "capability": "lint",
        "doc": "Additive JVM lint family aspect (Checkstyle/Pmd/SpotBugs/ktlint; SpotBugs target-coupled via JavaInfo).",
        "extra_attrs": {
            "_jacoco_runtime": attr.label(
                default = "@bazel_tools//tools/jdk:JacocoCoverage",
                providers = [JavaInfo],
                doc = "JaCoCo runtime SpotBugs needs to resolve coverage-instrumented classes.",
            ),
            "_spotbugs_effort": attr.label(default = "//config:spotbugs_effort"),
            "_spotbugs_threshold": attr.label(default = "//config:spotbugs_threshold"),
        },
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_jvm_typecheck": {
        "capability": "typecheck",
        "doc": "Additive JVM typecheck family aspect (delegated javac over rules_java targets, kotlinc over rules_kotlin targets, scalac over Scala 2.13 rules_scala targets).",
        "requires": [jvm_check_aspect],
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_scala_format": {
        "capability": "format",
        "doc": "Additive Scala format family aspect (scalafmt).",
        "shard": "scala",
        "suffix": "-scala",
    },
    "real_scala_lint": {
        "capability": "lint",
        "doc": "Additive Scala lint family aspect (Scalafix; target-coupled via SemanticdbInfo and JavaInfo).",
        "shard": "scala",
        "suffix": "-scala",
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

def _jvm_aux_jars(java_info, owned):
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
    auxiliary = _jvm_aux_jars(target[JavaInfo], owned)
    return (
        sorted(owned.values(), key = lambda f: f.path),
        sorted(auxiliary.values(), key = lambda f: f.path),
    )

def _spotbugs_hook(target, ctx, resolved, _capability, _what, _shared):
    if JavaInfo not in target:
        return without(resolved, "spotbugs")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None and ".." not in output.class_jar.short_path.split("/"):
            owned[output.class_jar.path] = output.class_jar
    auxiliary = _jvm_aux_jars(target[JavaInfo], owned)
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
    # The runner request has no spotbugs fields: effort travels as the edition, threshold as the dialect.
    options = {}
    if hasattr(ctx.attr, "_spotbugs_effort"):
        options = {
            "dialect": ctx.attr._spotbugs_threshold[BuildSettingInfo].value,
            "edition": ctx.attr._spotbugs_effort[BuildSettingInfo].value,
        }
    return real_context(tool_files = tool_files, inputs = inputs, options = options)

def _scalafix_hook(target, _ctx, _resolved, _capability, what, _shared):
    (owned, aux) = _scalafix_context(target, what)
    tool_files = []
    inputs = []
    for index, jar in enumerate(owned):
        stage(tool_files, inputs, "dx-scalafix/owned/%d-%s" % (index, jar.basename), jar)
    for index, jar in enumerate(aux):
        stage(tool_files, inputs, "dx-scalafix/aux/%d-%s" % (index, jar.basename), jar)
    return real_context(tool_files = tool_files, inputs = inputs)

def _jvm_check_hook(tool):
    def _hook(target, _ctx, resolved, _capability, _what, _shared):
        if JvmCheckInfo not in target or target[JvmCheckInfo].tool != tool:
            return without(resolved, tool)
        files = [target[JvmCheckInfo].log] + target[JvmCheckInfo].unused_deps
        return real_context(upstream = files, late_inputs = files)

    return _hook

HOOK_ORDER = ["spotbugs", "scalafix", "javac", "kotlinc", "scalac"]

HOOKS = {
    "spotbugs": _spotbugs_hook,
    "scalafix": _scalafix_hook,
    "javac": _jvm_check_hook("javac"),
    "kotlinc": _jvm_check_hook("kotlinc"),
    "scalac": _jvm_check_hook("scalac"),
}

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

real_jvm_format_aspect = real_make_aspect(_CFG, "real_jvm_format")

real_jvm_lint_aspect = real_make_aspect(_CFG, "real_jvm_lint")

real_jvm_typecheck_aspect = real_make_aspect(_CFG, "real_jvm_typecheck")

real_scala_format_aspect = real_make_aspect(_CFG, "real_scala_format")

real_scala_lint_aspect = real_make_aspect(_CFG, "real_scala_lint")
