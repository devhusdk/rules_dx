"""Scala real quality aspects."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//core/quality:family_hooks.bzl", "stage")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//java/private/quality:jvm.bzl", "jvm_aux_jars", "jvm_check_hook")
load(":scalac_check.bzl", "scalac_check_aspect")

TOOLS = {
    "scalac": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "scala",
    },
    "scalafix": {
        "attr": attr.label(default = "//config:tool_scalafix", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "scala",
    },
    "scalafmt": {
        "attr": attr.label(default = "//config:tool_scalafmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "heavy",
        "shard": "scala",
    },
}

SHARD_SPECS = {
    "real_scala_format": {
        "capability": "format",
        "doc": "Additive Scala format family aspect (scalafmt).",
        "shard": "scala",
    },
    "real_scala_lint": {
        "capability": "lint",
        "doc": "Additive Scala lint family aspect (Scalafix; target-coupled via SemanticdbInfo and JavaInfo).",
        "shard": "scala",
    },
    "real_scala_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Scala typecheck family aspect (delegated scalac over Scala 2.13 and 3.3+ rules_scala targets).",
        "requires": [scalac_check_aspect],
        "shard": "scala",
    },
}

def _scalafix_context(target, what):
    """Returns the semanticdb jars and the dependency jars scalafix reads for one target."""
    if SemanticdbInfo not in target or not target[SemanticdbInfo].semanticdb_enabled or not target[SemanticdbInfo].is_bundled_in_jar:
        fail(what + ": scalafix needs semanticdb bundled in the jar; add register_toolchains(\"@rules_dx//scala/private/toolchain/hub:all\") to MODULE.bazel, or set enable_semanticdb = True and semanticdb_bundle_in_jar = True on your own scala_toolchain")
    if JavaInfo not in target:
        fail(what + ": scalafix needs a JavaInfo target")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None:
            owned[output.class_jar.path] = output.class_jar
    if len(owned) == 0:
        fail(what + ": scalafix found no compiled jar for this target")
    auxiliary = jvm_aux_jars(target[JavaInfo], owned)
    return (
        sorted(owned.values(), key = lambda f: f.path),
        sorted(auxiliary.values(), key = lambda f: f.path),
    )

def _scalafix_hook(target, _ctx, _resolved, _capability, what, _shared):
    (owned, aux) = _scalafix_context(target, what)
    tool_files = []
    inputs = []
    for index, jar in enumerate(owned):
        stage(tool_files, inputs, "dx-scalafix/owned/%d-%s" % (index, jar.basename), jar)
    for index, jar in enumerate(aux):
        stage(tool_files, inputs, "dx-scalafix/aux/%d-%s" % (index, jar.basename), jar)
    return real_context(tool_files = tool_files, inputs = inputs)

HOOK_ORDER = ["scalafix", "scalac"]

HOOKS = {
    "scalafix": _scalafix_hook,
    "scalac": jvm_check_hook("scalac"),
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
)

real_scala_format_aspect = real_make_aspect(_CFG, "real_scala_format")

real_scala_lint_aspect = real_make_aspect(_CFG, "real_scala_lint")

real_scala_typecheck_aspect = real_make_aspect(_CFG, "real_scala_typecheck")

ScalaShardProbeInfo = provider(
    doc = "Shard names the scala shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_scala_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "scala", ScalaShardProbeInfo)

real_scala_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "scala", ScalaShardProbeInfo)

real_scala_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "scala", ScalaShardProbeInfo)
