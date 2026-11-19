"""Type-checks a JVM target with its compiler (javac, kotlinc, scalac) and records the diagnostics."""

load("@rules_java//java:java_utils.bzl", "utils")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common:java_plugin_info.bzl", "JavaPluginInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KspPluginInfo", "KtCompilerPluginInfo", "KtJvmInfo", "KtPluginConfiguration")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions", "kotlinc_options_to_flags")
load("@rules_scala//scala:providers.bzl", "DepsInfo", "ScalaInfo")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core/quality:sources.bzl", "single_target")

visibility(["//java/..."])

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"
_SCALA_TOOLCHAIN_TYPE = "@rules_scala//scala:toolchain_type"
_ANDROID_SDK_TOOLCHAIN_TYPE = "@rules_android//toolchains/android_sdk:toolchain_type"
_ANDROID_KINDS = ["android_library", "kt_android_library"]

# The check drivers speak Bazel's JSON worker protocol (java/private/quality/tools/worker), so local
# builds keep one warm JVM, and for kotlinc and scalac one loaded compiler, per mnemonic instead of a
# cold start per target. Singleplex: --worker_sandboxing in the generated bazelrc sandboxes it.
_WORKER = {
    "requires-worker-protocol": "json",
    "supports-workers": "1",
}

JvmAndroidSdkInfo = provider(
    doc = "Android platform jars an Android JVM target compiles against.",
    fields = {"jars": "depset[File]: the android.jar of the resolved Android SDK"},
)

def _jvm_android_sdk_impl(_target, ctx):
    if ctx.rule.kind not in _ANDROID_KINDS:
        return []
    toolchain = ctx.toolchains[_ANDROID_SDK_TOOLCHAIN_TYPE]
    if toolchain == None or getattr(toolchain, "android_sdk_info", None) == None:
        fail("%s: %s needs a resolved Android SDK toolchain for quality checks" % (ctx.label, ctx.rule.kind))
    return [JvmAndroidSdkInfo(jars = depset([toolchain.android_sdk_info.android_jar]))]

jvm_android_sdk_aspect = aspect(
    implementation = _jvm_android_sdk_impl,
    doc = "Exposes the android.jar of the resolved Android SDK for android_library and kt_android_library targets.",
    toolchains = [config_common.toolchain_type(_ANDROID_SDK_TOOLCHAIN_TYPE, mandatory = False)],
)

def android_sdk_jars(target):
    """Returns the Android platform jars of one target as a depset."""
    if JvmAndroidSdkInfo in target:
        return target[JvmAndroidSdkInfo].jars
    return depset()

# The scalac driver supports Scala 2.13 and Scala 3.3 or newer (-Wunused does not exist before 3.3); targets on other versions get no result.
_SCALAC_VERSION_PREFIX = "2.13."

def _is_scala3(version):
    parts = version.split(".")
    return len(parts) >= 2 and parts[0] == "3" and parts[1].isdigit() and int(parts[1]) >= 3

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
    doc = "Output of one non-failing JVM compiler check (javac, kotlinc and scalac).",
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

def _unused_deps_map(target, ctx, attrs, implicit = []):
    """Writes the direct dependencies with their compile jars, the known jars and the transitive classpath."""
    known = [android_sdk_jars(target)] + implicit
    toolchain = ctx.toolchains[_KT_TOOLCHAIN_TYPE]
    if toolchain:
        known.append(toolchain.jvm_stdlibs.transitive_compile_time_jars)
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
    return [_unused_deps_map(target, ctx, attrs), jdeps[0]]

def _plugin_param(values):
    return ";".join([_plugin_escape(v) for v in values])

def _plugin_escape(value):
    return value.replace(";", "\\;")

def _analyzer_jar(jar):
    return _plugin_escape(jar.path) if jar.owner else None

def _analyzer_label(jar):
    return _plugin_escape(str(jar.owner).removesuffix("_upstream")) if jar.owner else None

def _analyzer_path(jar):
    return _plugin_escape(jar.path)

def _analyzer_library(jar):
    return "direct=%s\t%s" % (_analyzer_jar(jar), _analyzer_label(jar)) if jar.owner else None

def _add_joined_option(args, name, listed, jars, map_each):
    """Adds one analyzer list option whose items are `listed` followed by `jars` mapped through `map_each`."""
    prefix = "scalacopt=-P:dependency-analyzer:%s:" % name
    head = (_plugin_param(listed) + ";").replace("%", "%%") if listed else ""
    args.add_joined(jars, join_with = ";", map_each = map_each, format_joined = prefix + head + "%s", omit_if_empty = False)

def _analyzer(target, ctx, attrs, library):
    """The rules_scala dependency analyzer jars and options for a scala target, or None.

    The scala library jars are direct deps that are never reported unused. The indirect set holds the
    whole transitive classpath of the deps, direct jars included.
    """
    plugin = ctx.attr._dependency_analyzer
    if JavaInfo not in plugin:
        return None
    deps = [d for d in getattr(attrs, "deps", []) if JavaInfo in d]

    # The indirect set holds every direct jar, so it is empty only when there are no jars at all.
    indirect = depset(transitive = [d[JavaInfo].transitive_compile_time_jars for d in deps])
    if not indirect:
        return None
    info = plugin[JavaInfo]
    jars = depset(info.runtime_output_jars, transitive = [info.transitive_runtime_jars], order = "preorder")
    ignored = [str(t.label) for t in getattr(attrs, "exports", []) + getattr(attrs, "unused_dependency_checker_ignored_targets", [])]
    prefix = "-P:dependency-analyzer:"
    options = [
        prefix + "unused-deps-mode:warn",
        prefix + "strict-deps-mode:warn",
        prefix + "dependency-tracking-method:ast",
        prefix + "current-target:" + str(target.label),
    ]
    return struct(
        jars = jars,
        options = options,
        direct = [(_plugin_escape(str(d.label)), d[JavaInfo].compile_jars) for d in deps],
        ignored = ignored,
        library = library,
        indirect = indirect,
    )

def _add_analyzer_options(args, analyzer):
    args.add_all(analyzer.options, format_each = "scalacopt=%s")

    # The driver joins the direct= lines into the direct-jars and direct-targets options. Each line pairs a jar
    # with the dep that brings it, so a dep's exported jars count as that dep.
    for (label, jars) in analyzer.direct:
        args.add_all(jars, map_each = _analyzer_path, format_each = "direct=%s\t" + label.replace("%", "%%"))
    args.add_all(analyzer.library, map_each = _analyzer_library)
    _add_joined_option(args, "indirect-jars", [], analyzer.indirect, _analyzer_jar)
    _add_joined_option(args, "indirect-targets", [], analyzer.indirect, _analyzer_label)
    _add_joined_option(args, "unused-deps-ignored-targets", analyzer.ignored, analyzer.library, _analyzer_label)

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
        execution_requirements = _WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    return [JvmCheckInfo(log = log, tool = "kotlinc", unused_deps = _unused_deps_files(target, ctx, attrs))]

def _deps_info(toolchain, deps_id):
    for provider in getattr(toolchain, "dep_providers", []):
        if DepsInfo in provider and provider[DepsInfo].deps_id == deps_id:
            return provider[DepsInfo]
    return None

def scalac_toolchain_jars(toolchain):
    """The jars the scalac driver needs from a rules_scala toolchain."""
    if toolchain == None:
        return None
    scala3 = _is_scala3(toolchain.scala_version)
    if not scala3 and not toolchain.scala_version.startswith(_SCALAC_VERSION_PREFIX):
        return None
    compile_deps = _deps_info(toolchain, "scala_compile_classpath")
    library_deps = _deps_info(toolchain, "scala_library_classpath")
    if compile_deps == None or library_deps == None:
        return None
    compile_infos = [d[JavaInfo] for d in compile_deps.deps if JavaInfo in d]
    library_infos = [d[JavaInfo] for d in library_deps.deps if JavaInfo in d]
    if not compile_infos or not library_infos:
        return None
    return struct(
        compiler = depset(transitive = [i.transitive_runtime_jars for i in compile_infos]),
        library = depset(transitive = [i.transitive_compile_time_jars for i in library_infos]),
        scala3 = scala3,
    )

def _scala_plugin_jars(targets):
    return [depset(transitive = [t[JavaInfo].transitive_runtime_jars]) if JavaInfo in t else t.files for t in targets]

def _scalac_check(target, ctx, attrs):
    toolchain = ctx.toolchains[_SCALA_TOOLCHAIN_TYPE]
    jars = scalac_toolchain_jars(toolchain)
    if jars == None or not hasattr(ctx.rule.files, "srcs"):
        return []
    sources = [f for f in ctx.rule.files.srcs if f.extension in ["scala", "java"]]
    srcjars = [f for f in ctx.rule.files.srcs if f.extension == "srcjar"]
    if not [f for f in sources if f.extension == "scala"] and not srcjars:
        return []
    options = ["-Wunused:all" if jars.scala3 else "-Wunused:_"] + list(toolchain.scalacopts) + list(getattr(attrs, "scalacopts", []))

    options.append("-Wconf:cat=unused:w")

    if [o for o in options if "$(" in o]:
        return []
    deps = [d for d in getattr(attrs, "deps", []) if JavaInfo in d]
    classpath = depset(transitive = [d[JavaInfo].transitive_compile_time_jars for d in deps] + [jars.library])
    plugin_targets = list(toolchain.plugins) + list(getattr(attrs, "plugins", []))
    plugin_jars = depset(transitive = _scala_plugin_jars(plugin_targets))

    analyzer = None if jars.scala3 else _analyzer(target, ctx, attrs, jars.library)
    analyzer_jars = analyzer.jars if analyzer else depset()

    log = ctx.actions.declare_file("dx_scalac/%s.json" % target.label.name)
    args = ctx.actions.args()
    args.set_param_file_format("multiline")
    args.use_param_file("@%s", use_always = True)
    args.add(log, format = "output=%s")
    args.add_all(sources, format_each = "source=%s")
    args.add_all(srcjars, format_each = "srcjar=%s")
    args.add_all(classpath, format_each = "classpath=%s")
    args.add_all(jars.compiler, format_each = "compiler=%s")
    args.add_all(plugin_jars, format_each = "plugin=%s")
    args.add_all(analyzer_jars, format_each = "analyzer=%s")
    args.add_all(options, format_each = "scalacopt=%s")
    if analyzer:
        _add_analyzer_options(args, analyzer)
    ctx.actions.run(
        mnemonic = "ScalacCheck",
        progress_message = "scalac check %{label}",
        executable = ctx.attr._scalac_check.files_to_run,
        arguments = [args],
        inputs = depset(sources + srcjars, transitive = [classpath, jars.compiler, plugin_jars, analyzer_jars]),
        outputs = [log],
        execution_requirements = _WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    unused_deps = [_unused_deps_map(target, ctx, attrs, [jars.library])] if analyzer and "dx-keep" not in attrs.tags else []
    return [JvmCheckInfo(log = log, tool = "scalac", unused_deps = unused_deps)]

def _jvm_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = single_target(attrs.upstream)
        if JvmCheckInfo not in upstream:
            return []
        return [upstream[JvmCheckInfo]]
    if KtJvmInfo in target:
        return _kotlinc_check(target, ctx, attrs)
    if ScalaInfo in target:
        return _scalac_check(target, ctx, attrs)
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
        execution_requirements = _WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    return [JvmCheckInfo(log = log, tool = "javac", unused_deps = _unused_deps_files(target, ctx, attrs))]

jvm_check_aspect = aspect(
    implementation = _jvm_check_impl,
    doc = "Runs javac (rules_java), kotlinc (rules_kotlin) or scalac (rules_scala, Scala 2.13 and 3.3+) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    requires = [jvm_android_sdk_aspect],
    toolchains = [
        config_common.toolchain_type(_KT_TOOLCHAIN_TYPE, mandatory = False),
        config_common.toolchain_type(_SCALA_TOOLCHAIN_TYPE, mandatory = False),
    ],
    attrs = {
        "_kotlinc_check": attr.label(
            default = "@dx_prebuilt//:kotlinc_check",
            executable = True,
            cfg = "exec",
        ),
        "_scalac_check": attr.label(
            default = "@dx_prebuilt//:scalac_check",
            executable = True,
            cfg = "exec",
        ),
        "_dependency_analyzer": attr.label(
            default = "@rules_scala//third_party/dependency_analyzer/src/main:dependency_analyzer",
            cfg = "exec",
        ),
        "_javac_check": attr.label(
            default = "@dx_prebuilt//:javac_check",
            executable = True,
            cfg = "exec",
        ),
    },
)
