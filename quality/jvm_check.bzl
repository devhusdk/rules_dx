"""Type-checks a JVM target with its compiler (javac, kotlinc, scalac) and records the diagnostics."""

load("@rules_java//java:java_utils.bzl", "utils")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common:java_plugin_info.bzl", "JavaPluginInfo")
load("@rules_kotlin//kotlin/internal:defs.bzl", "KspPluginInfo", "KtCompilerPluginInfo", "KtJvmInfo", "KtPluginConfiguration")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions", "kotlinc_options_to_flags")
load("@rules_scala//scala:providers.bzl", "DepsInfo", "ScalaInfo")
load("//libs/starlark:resources.bzl", "dx_heavy_resources")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"
_SCALA_TOOLCHAIN_TYPE = "@rules_scala//scala:toolchain_type"

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

def _indirect_jars(target, direct):
    """The transitive compile jars that are not in the `direct` path set, with their owner labels."""
    indirect = []
    for jar in target[JavaInfo].transitive_compile_time_jars.to_list():
        if jar.path not in direct and jar.owner and jar.owner != target.label:
            indirect.append({"jar": jar.path, "label": str(jar.owner).removesuffix("_upstream")})
    return indirect

def _unused_deps_map(target, ctx, attrs, implicit = []):
    """Writes the direct dependencies with their compile jars and the transitive ones the rule does not list.

    `implicit` holds jar paths the compiler may use without a dep, such as the Scala library.
    """
    deps = []
    direct = {}
    for dep in getattr(attrs, "deps", []):
        if JavaInfo not in dep:
            continue
        jars = [j.path for j in dep[JavaInfo].compile_jars.to_list()]
        for jar in jars:
            direct[jar] = True
        if jars:
            deps.append({"label": str(dep.label), "jars": jars})
    toolchain = ctx.toolchains[_KT_TOOLCHAIN_TYPE]
    if toolchain:
        for jar in toolchain.jvm_stdlibs.transitive_compile_time_jars.to_list():
            direct[jar.path] = True
    for path in implicit:
        direct[path] = True
    indirect = _indirect_jars(target, direct)
    name = target.label.name.removesuffix("_upstream")
    out = ctx.actions.declare_file("dx_jvm_deps/%s.unused_deps.json" % target.label.name)
    ctx.actions.write(out, json.encode({"package": target.label.package, "target": name, "deps": deps, "indirect": indirect}))
    return out

def _unused_deps_files(target, ctx, attrs):
    """Files the runner reads to find direct deps whose jars the compiler never used."""
    jdeps = [o.jdeps for o in target[JavaInfo].java_outputs if o.jdeps]
    if not jdeps:
        return []
    return [_unused_deps_map(target, ctx, attrs), jdeps[0]]

def _plugin_param(values):
    return ";".join([v.replace(";", "\\;") for v in values])

def _analyzer(target, ctx, attrs, implicit):
    """The rules_scala dependency analyzer jars and options for a scala target, or None.

    `implicit` holds the jar paths of the Scala library, which a target may use without listing it.
    """
    plugin = ctx.attr._dependency_analyzer
    if JavaInfo not in plugin:
        return None
    direct = [d for d in getattr(attrs, "deps", []) if JavaInfo in d and d[JavaInfo].compile_jars.to_list()]
    direct_jars = []
    direct_targets = []
    for dep in direct:
        for jar in dep[JavaInfo].compile_jars.to_list():
            direct_jars.append(jar.path)
            direct_targets.append(str(dep.label))
    known = {path: True for path in direct_jars + implicit}
    indirect = _indirect_jars(target, known)
    if not direct and not indirect:
        return None
    info = plugin[JavaInfo]
    jars = []
    for jar in info.runtime_output_jars + info.transitive_runtime_jars.to_list():
        if jar not in jars:
            jars.append(jar)
    ignored = [str(t.label) for t in getattr(attrs, "exports", []) + getattr(attrs, "unused_dependency_checker_ignored_targets", [])]
    prefix = "-P:dependency-analyzer:"
    options = [
        prefix + "unused-deps-mode:warn",
        prefix + "strict-deps-mode:warn",
        prefix + "dependency-tracking-method:ast",
        prefix + "current-target:" + str(target.label),
        prefix + "direct-jars:" + _plugin_param(direct_jars),
        prefix + "direct-targets:" + _plugin_param(direct_targets),
        prefix + "indirect-jars:" + _plugin_param([i["jar"] for i in indirect]),
        prefix + "indirect-targets:" + _plugin_param([i["label"] for i in indirect]),
    ]
    if ignored:
        options.append(prefix + "unused-deps-ignored-targets:" + _plugin_param(ignored))
    return struct(jars = jars, options = options)

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
    """The jars the scalac driver needs from a rules_scala toolchain.

    Args:
        toolchain: the resolved `@rules_scala//scala:toolchain_type` ToolchainInfo, or None.

    Returns:
        None when the toolchain is absent, not Scala 2.13 or 3.3+, or lacks a classpath; otherwise a
        struct with `compiler` (runtime jars of the compiler, never ijars), `library` (compile jars)
        and `scala3` (whether the toolchain is Scala 3).
    """
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

    # The semanticdb toolchain demotes unused warnings to info for scalafix; the last -Wconf wins, so this restores them.
    options.append("-Wconf:cat=unused:w")

    # Make variables and $(location) in scalacopts are not expanded here, so the options would be wrong.
    if [o for o in options if "$(" in o]:
        return []
    deps = [d for d in getattr(attrs, "deps", []) if JavaInfo in d]
    classpath = depset(transitive = [d[JavaInfo].transitive_compile_time_jars for d in deps] + [jars.library])
    plugin_targets = list(toolchain.plugins) + list(getattr(attrs, "plugins", []))
    plugin_jars = depset(transitive = _scala_plugin_jars(plugin_targets))

    # The dependency analyzer is a Scala 2 compiler plugin.
    implicit = [j.path for j in jars.library.to_list()]
    analyzer = None if jars.scala3 else _analyzer(target, ctx, attrs, implicit)
    analyzer_jars = []
    if analyzer:
        options = options + analyzer.options
        analyzer_jars = analyzer.jars

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
    ctx.actions.run(
        mnemonic = "ScalacCheck",
        progress_message = "scalac check %{label}",
        executable = ctx.attr._scalac_check.files_to_run,
        arguments = [args],
        inputs = depset(sources + srcjars + analyzer_jars, transitive = [classpath, jars.compiler, plugin_jars]),
        outputs = [log],
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    unused_deps = [_unused_deps_map(target, ctx, attrs, implicit)] if analyzer else []
    return [JvmCheckInfo(log = log, tool = "scalac", unused_deps = unused_deps)]

def _jvm_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = _first(attrs.upstream)
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
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    return [JvmCheckInfo(log = log, tool = "javac", unused_deps = _unused_deps_files(target, ctx, attrs))]

jvm_check_aspect = aspect(
    implementation = _jvm_check_impl,
    doc = "Runs javac (rules_java), kotlinc (rules_kotlin) or scalac (rules_scala, Scala 2.13 and 3.3+) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    toolchains = [
        config_common.toolchain_type(_KT_TOOLCHAIN_TYPE, mandatory = False),
        config_common.toolchain_type(_SCALA_TOOLCHAIN_TYPE, mandatory = False),
    ],
    attrs = {
        "_kotlinc_check": attr.label(
            default = "//quality/tools/jvm/kotlinc_check:kotlinc_check",
            executable = True,
            cfg = "exec",
        ),
        "_scalac_check": attr.label(
            default = "//quality/tools/jvm/scalac_check:scalac_check",
            executable = True,
            cfg = "exec",
        ),
        # Built for the exec platform: its Java sources do not compile under the repo's -Werror javacopts.
        "_dependency_analyzer": attr.label(
            default = "@rules_scala//third_party/dependency_analyzer/src/main:dependency_analyzer",
            cfg = "exec",
        ),
        "_javac_check": attr.label(
            default = "//quality/tools/jvm/javac_check:javac_check",
            executable = True,
            cfg = "exec",
        ),
    },
)
