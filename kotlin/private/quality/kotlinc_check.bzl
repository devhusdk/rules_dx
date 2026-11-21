"""Type-checks a Kotlin target with kotlinc and records the diagnostics."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common:java_plugin_info.bzl", "JavaPluginInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KspPluginInfo", "KtCompilerPluginInfo", "KtJvmInfo", "KtPluginConfiguration")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions", "kotlinc_options_to_flags")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core/quality:sources.bzl", "single_target")
load("//java/private/quality:jvm_check.bzl", "JVM_CHECK_WORKER", "JvmCheckInfo", "unused_deps_map")
load("//java/private/quality:jvm_info.bzl", "JvmAndroidSdkInfo", "android_sdk_jars")

visibility(["//kotlin/..."])

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

def _unused_deps_files(target, ctx, attrs, toolchain):
    """Files the runner reads to find direct deps whose jars kotlinc never used."""
    jdeps = [o.jdeps for o in target[JavaInfo].java_outputs if o.jdeps]
    if not jdeps or "dx-keep" in attrs.tags:
        return []
    return [unused_deps_map(target, ctx, attrs, [android_sdk_jars(target)], [toolchain.jvm_stdlibs]), jdeps[0]]

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
            # Bounded by the direct deps' exported plugins: each needs its Target providers.
            targets += dep[KtJvmInfo].exported_compiler_plugins.to_list()
    plugins = {}
    configs = {}
    for t in targets:
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

def _kotlinc_jar(f):
    return f.path if f.basename in _KOTLINC_JARS and f.dirname.endswith("/lib") else None

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
    kotlin_home = toolchain.kotlin_home.files
    deps = [d for d in getattr(attrs, "deps", []) + getattr(attrs, "associates", []) if JavaInfo in d]
    classpath = depset(
        transitive = [d[JavaInfo].transitive_compile_time_jars for d in deps] +
                     [toolchain.jvm_stdlibs.transitive_compile_time_jars, android_sdk_jars(target)],
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
    args.add_all(kotlin_home, map_each = _kotlinc_jar, format_each = "compiler=%s")
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
            sources + srcjars,
            transitive = [kotlin_home, classpath, plugin_jars, friends],
        ),
        outputs = [log],
        execution_requirements = JVM_CHECK_WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    return [JvmCheckInfo(log = log, tool = "kotlinc", unused_deps = _unused_deps_files(target, ctx, attrs, toolchain))]

def _kotlinc_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = single_target(attrs.upstream)
        if JvmCheckInfo not in upstream:
            return []
        return [upstream[JvmCheckInfo]]
    if KtJvmInfo not in target:
        return []
    return _kotlinc_check(target, ctx, attrs)

kotlinc_check_aspect = aspect(
    implementation = _kotlinc_check_impl,
    doc = "Runs kotlinc (rules_kotlin) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    required_aspect_providers = [[JvmAndroidSdkInfo]],
    toolchains = [config_common.toolchain_type(_KT_TOOLCHAIN_TYPE, mandatory = False)],
    attrs = {
        "_kotlinc_check": attr.label(
            default = "@dx_prebuilt//:kotlinc_check",
            executable = True,
            cfg = "exec",
        ),
    },
)
