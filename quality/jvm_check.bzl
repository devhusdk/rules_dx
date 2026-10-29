"""Type-checks a JVM target with its compiler (javac, kotlinc) and records the diagnostics."""

load("@rules_java//java:java_utils.bzl", "utils")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common:java_plugin_info.bzl", "JavaPluginInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KspPluginInfo", "KtCompilerPluginInfo", "KtJvmInfo", "KtPluginConfiguration")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions", "kotlinc_options_to_flags")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

# The jars of the Kotlin compiler distribution the kotlinc driver loads, in the toolchain's kotlin_home.
_KOTLINC_JARS = [
    "annotations-13.0.jar",
    "kotlin-compiler.jar",
    "kotlin-daemon.jar",
    "kotlin-reflect.jar",
    "kotlin-script-runtime.jar",
    "kotlin-stdlib.jar",
    "kotlinx-coroutines-core-jvm.jar",
]

JvmCheckInfo = provider(
    doc = "Output of one non-failing JVM compiler check (javac and kotlinc today; scalac shares it).",
    fields = {
        "log": "File: versioned JSON log with the tool, its exit status and its diagnostics.",
        "tool": "string: the registry tool id that produced the log.",
    },
)

# Flags the Bazel javac toolchain passes to JavaBuilder that plain javac rejects or that run ErrorProne.
_JAVABUILDER_ONLY_PREFIXES = ["-Xep", "-Xplugin:ErrorProne", "--should-stop=", "-XDshould-stop"]

def _first(value):
    return value[0] if type(value) == "list" else value

def javac_options(tokens):
    """Drops the toolchain flags that plain javac cannot take.

    Args:
        tokens: [str] tokenized javacopts.

    Returns:
        [str] the options javac itself accepts.
    """
    return [t for t in tokens if not any([t.startswith(p) for p in _JAVABUILDER_ONLY_PREFIXES])]

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

def _kotlinc_flags(toolchain, opts):
    """The kotlinc flags rules_kotlin compiles with: the target's options replace the toolchain's."""
    flags = list(kotlinc_options_to_flags(opts)) if opts else []
    for (value, toolchain_value, flag) in [
        (getattr(opts, "jvm_target", None), toolchain.jvm_target, "-jvm-target"),
        (getattr(opts, "api_version", None), toolchain.api_version, "-api-version"),
        (getattr(opts, "language_version", None), toolchain.language_version, "-language-version"),
    ]:
        if not value and toolchain_value:
            flags += [flag, toolchain_value]
    return flags

def _kotlin_plugins(attrs):
    """Compiler plugin jars and options of a Kotlin target, or None when it needs a processor stage."""
    targets = list(getattr(attrs, "plugins", []))
    for dep in getattr(attrs, "deps", []):
        if KtJvmInfo in dep:
            targets += dep[KtJvmInfo].exported_compiler_plugins.to_list()
    plugins = {}
    configs = {}
    for t in targets:
        # kapt and KSP generate sources the check cannot see, so it would report false errors.
        if JavaPluginInfo in t or KspPluginInfo in t:
            return None
        if KtCompilerPluginInfo in t:
            plugins[t[KtCompilerPluginInfo].id] = t[KtCompilerPluginInfo]
        if KtPluginConfiguration in t:
            configs.setdefault(t[KtPluginConfiguration].id, []).append(t[KtPluginConfiguration])
    jars = []
    options = []
    for plugin in plugins.values():
        if not plugin.compile:
            continue
        classpath = [plugin.classpath]
        opts = list(plugin.options)
        if plugin.id in configs:
            cfg = plugin.merge_cfgs(plugin, configs[plugin.id])
            classpath.append(cfg.classpath)
            opts += cfg.options
        jars.append(depset(transitive = classpath))
        options += ["plugin:%s:%s=%s" % (plugin.id, o.key, o.value) for o in opts]
    return (depset(transitive = jars), options)

def _kotlinc_check(target, ctx, attrs):
    toolchain = ctx.toolchains[_KT_TOOLCHAIN_TYPE]
    if toolchain == None or not hasattr(ctx.rule.files, "srcs"):
        return []
    sources = [f for f in ctx.rule.files.srcs if f.extension in ["kt", "kts", "java"]]
    srcjars = [f for f in ctx.rule.files.srcs if f.extension == "srcjar"]
    if not [f for f in sources if f.extension != "java"] and not srcjars:
        return []
    plugin_data = _kotlin_plugins(attrs)
    if plugin_data == None:
        return []
    (plugin_jars, plugin_options) = plugin_data
    kotlin_home = [f for f in toolchain.kotlin_home.files.to_list() if f.basename in _KOTLINC_JARS and f.dirname.endswith("/lib")]
    if not [f for f in kotlin_home if f.basename == "kotlin-compiler.jar"]:
        return []
    deps = [d for d in getattr(attrs, "deps", []) + getattr(attrs, "associates", []) if JavaInfo in d]
    classpath = depset(
        transitive = [d[JavaInfo].transitive_compile_time_jars for d in deps] +
                     [toolchain.jvm_stdlibs.transitive_compile_time_jars],
    )
    friends = target[KtJvmInfo].module_jars
    opts = attrs.kotlinc_opts[KotlincOptions] if getattr(attrs, "kotlinc_opts", None) else toolchain.kotlinc_options
    flags = _kotlinc_flags(toolchain, opts)

    log = ctx.actions.declare_file("dx_kotlinc/%s.json" % target.label.name)
    args = ctx.actions.args()
    args.set_param_file_format("multiline")
    args.use_param_file("@%s", use_always = True)
    args.add(log, format = "output=%s")
    args.add_all(sources, format_each = "source=%s")
    args.add_all(srcjars, format_each = "srcjar=%s")
    args.add_all(classpath, format_each = "classpath=%s")
    args.add_all(kotlin_home, format_each = "compiler=%s")
    args.add_all(plugin_jars, format_each = "plugin=%s")
    args.add_all(plugin_options, format_each = "pluginopt=%s")
    args.add_all(friends, format_each = "friend=%s")
    args.add_all(flags, format_each = "kotlincopt=%s")
    args.add(target[KtJvmInfo].module_name, format = "module=%s")
    ctx.actions.run(
        mnemonic = "KotlincCheck",
        progress_message = "kotlinc check %{label}",
        executable = ctx.attr._kotlinc_check.files_to_run,
        arguments = [args],
        inputs = depset(
            sources + srcjars + kotlin_home,
            transitive = [classpath, plugin_jars, friends],
        ),
        outputs = [log],
        resource_set = dx_quality_heavy_resources,
    )
    return [JvmCheckInfo(log = log, tool = "kotlinc")]

def _jvm_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = _first(attrs.upstream)
        if JvmCheckInfo not in upstream:
            return []
        return [upstream[JvmCheckInfo]]
    if KtJvmInfo in target:
        return _kotlinc_check(target, ctx, attrs)
    if JavaInfo not in target or target[JavaInfo].compilation_info == None or not hasattr(ctx.rule.files, "srcs"):
        return []
    sources = [f for f in ctx.rule.files.srcs if f.extension == "java"]
    srcjars = [f for f in ctx.rule.files.srcs if f.extension == "srcjar"]
    if not sources and not srcjars:
        return []
    info = target[JavaInfo].compilation_info
    (classes, processor_jars, processor_data) = _plugin_data(target, attrs)
    classpath = _depset(info.compilation_classpath)
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
        resource_set = dx_quality_heavy_resources,
    )
    return [JvmCheckInfo(log = log, tool = "javac")]

jvm_check_aspect = aspect(
    implementation = _jvm_check_impl,
    doc = "Runs javac (rules_java) or kotlinc (rules_kotlin) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    toolchains = [config_common.toolchain_type(_KT_TOOLCHAIN_TYPE, mandatory = False)],
    attrs = {
        "_kotlinc_check": attr.label(
            default = "//quality/tools/jvm/kotlinc_check:kotlinc_check",
            executable = True,
            cfg = "exec",
        ),
        "_javac_check": attr.label(
            default = "//quality/tools/jvm/javac_check:javac_check",
            executable = True,
            cfg = "exec",
        ),
    },
)
