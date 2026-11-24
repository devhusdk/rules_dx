"""Type-checks a Java target with javac and records the diagnostics."""

load("@rules_java//java:java_utils.bzl", "utils")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common:java_plugin_info.bzl", "JavaPluginInfo")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core/quality:sources.bzl", "single_target")
load("//java/private/quality:jvm_info.bzl", "JvmAndroidSdkInfo", "android_sdk_jars")

visibility(["//android/...", "//java/...", "//kotlin/...", "//scala/..."])

# The check drivers speak Bazel's JSON worker protocol (java/private/quality/tools/worker), so local
# builds keep one warm JVM, and for kotlinc and scalac one loaded compiler, per mnemonic instead of a
# cold start per target. Singleplex: --worker_sandboxing in the generated bazelrc sandboxes it.
JVM_CHECK_WORKER = {
    "requires-worker-protocol": "json",
    "supports-workers": "1",
}

JvmCheckInfo = provider(
    doc = "Output of one non-failing JVM compiler check (javac, kotlinc or scalac).",
    fields = {
        "log": "File: JSON log with the tool, its exit status and its diagnostics.",
        "tool": "string: the registry tool id that produced the log.",
        "unused_deps": "[File]: the direct dependency map and the compiler's jdeps file, or empty when the target has no jdeps.",
    },
)

# Flags the Bazel javac toolchain passes to JavaBuilder that plain javac rejects or that run ErrorProne.
_JAVABUILDER_ONLY_PREFIXES = ["-Xep", "-Xplugin:ErrorProne", "--should-stop=", "-XDshould-stop"]

def javac_options(tokens):
    """Drops the toolchain flags that plain javac cannot take."""
    return [t for t in tokens if not any([t.startswith(p) for p in _JAVABUILDER_ONLY_PREFIXES])]

def _known_line(jar):
    return json.encode({"known": jar.path})

def _jar_json(jar):
    return json.encode(jar.path)

def _classpath_line(jar):
    if not jar.owner:
        return None
    return json.encode({"jar": jar.path, "label": str(jar.owner)})

def unused_deps_map(target, ctx, attrs, known, known_infos = []):
    """Writes the direct dependencies with their compile jars, the known jars and the transitive classpath."""
    known = known + [info.transitive_compile_time_jars for info in known_infos]
    name = target.label.name.removesuffix("_upstream")
    header = {"package": target.label.package, "target": name, "label": str(target.label)}

    # Args expand the depsets only when the file is written, so analysis never flattens the classpath.
    content = ctx.actions.args()
    content.set_param_file_format("multiline")
    content.add(json.encode(header))
    for dep in getattr(attrs, "deps", []):
        if JavaInfo not in dep:
            continue
        line = '{"dep": %s, "jars": [%%s]}' % json.encode(str(dep.label)).replace("%", "%%")
        content.add_joined(dep[JavaInfo].compile_jars, map_each = _jar_json, join_with = ",", format_joined = line, omit_if_empty = True)
    content.add_all(depset(transitive = known), map_each = _known_line)
    content.add_all(target[JavaInfo].transitive_compile_time_jars, map_each = _classpath_line)
    out = ctx.actions.declare_file("dx_jvm_deps/%s.unused_deps.jsonl" % target.label.name)
    ctx.actions.write(out, content)
    return out

def _unused_deps_files(target, ctx, attrs):
    """Files the runner reads to find direct deps whose jars the compiler never used."""
    jdeps = [o.jdeps for o in target[JavaInfo].java_outputs if o.jdeps]
    if not jdeps or "dx-keep" in attrs.tags:
        return []
    return [unused_deps_map(target, ctx, attrs, [android_sdk_jars(target)]), jdeps[0]]

def _depset(value):
    return value if type(value) == "depset" else depset(value)

def _plugin_data(target, attrs):
    infos = [p[JavaPluginInfo].plugins for p in getattr(attrs, "plugins", []) if JavaPluginInfo in p]
    infos += [d[JavaInfo].plugins for d in getattr(attrs, "deps", []) if JavaInfo in d]
    infos.append(target[JavaInfo].plugins)
    classes = depset(transitive = [i.processor_classes for i in infos])
    jars = depset(transitive = [i.processor_jars for i in infos])
    data = depset(transitive = [i.processor_data for i in infos])
    return (classes, jars, data)

def _jvm_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = single_target(attrs.upstream)
        if JvmCheckInfo not in upstream:
            return []
        return [upstream[JvmCheckInfo]]
    if ctx.rule.kind.startswith(("scala_", "kt_")):
        return []
    if JavaInfo not in target or target[JavaInfo].compilation_info == None or not hasattr(ctx.rule.files, "srcs"):
        return []
    sources = [f for f in ctx.rule.files.srcs if f.extension == "java"]
    srcjars = [f for f in ctx.rule.files.srcs if f.extension == "srcjar"]
    if not sources and not srcjars:
        return []
    info = target[JavaInfo].compilation_info
    (classes, processor_jars, processor_data) = _plugin_data(target, attrs)
    classpath = depset(transitive = [_depset(info.compilation_classpath), android_sdk_jars(target)])
    boot_classpath = _depset(info.boot_classpath)
    options = javac_options(utils.tokenize_javacopts(ctx, info.javac_options))

    log = ctx.actions.declare_file("dx_javac/%s.json" % target.label.name)
    args = ctx.actions.args()
    args.set_param_file_format("multiline")
    args.use_param_file("@%s", use_always = True)
    args.add(log, format = "output=%s")
    args.add_all(sources, format_each = "source=%s")
    args.add_all(srcjars, format_each = "srcjar=%s")
    args.add_all(classpath, format_each = "classpath=%s")
    args.add_all(boot_classpath, format_each = "bootclasspath=%s")
    args.add_all(processor_jars, format_each = "processorpath=%s")
    args.add_all(classes, format_each = "processor=%s")
    args.add_all(options, format_each = "javacopt=%s")
    ctx.actions.run(
        mnemonic = "JavacCheck",
        progress_message = "javac check %{label}",
        executable = ctx.attr._javac_check.files_to_run,
        arguments = [args],
        inputs = depset(
            sources + srcjars,
            transitive = [classpath, boot_classpath, processor_jars, processor_data],
        ),
        outputs = [log],
        execution_requirements = JVM_CHECK_WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    return [JvmCheckInfo(log = log, tool = "javac", unused_deps = _unused_deps_files(target, ctx, attrs))]

jvm_check_aspect = aspect(
    implementation = _jvm_check_impl,
    doc = "Runs javac (rules_java) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    required_aspect_providers = [[JvmAndroidSdkInfo]],
    attrs = {
        "_javac_check": attr.label(
            default = "@dx_prebuilt//:javac_check",
            executable = True,
            cfg = "exec",
        ),
    },
)
