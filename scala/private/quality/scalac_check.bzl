"""Type-checks a Scala target with scalac and records the diagnostics."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_scala//scala:providers.bzl", "DepsInfo", "ScalaInfo")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core/quality:sources.bzl", "single_target")
load("//java/private/quality:jvm_check.bzl", "JVM_CHECK_WORKER", "JvmCheckInfo", "unused_deps_map")

visibility(["//scala/..."])

SCALA_TOOLCHAIN_TYPE = "@rules_scala//scala:toolchain_type"

# The scalac driver supports Scala 2.13 and Scala 3.3 or newer (-Wunused does not exist before 3.3); targets on other versions get no result.
_SCALAC_VERSION_PREFIX = "2.13."

def _is_scala3(version):
    parts = version.split(".")
    return len(parts) >= 2 and parts[0] == "3" and parts[1].isdigit() and int(parts[1]) >= 3

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
    toolchain = ctx.toolchains[SCALA_TOOLCHAIN_TYPE]
    jars = scalac_toolchain_jars(toolchain)
    if jars == None or not hasattr(ctx.rule.files, "srcs"):
        return []
    sources = [f for f in ctx.rule.files.srcs if f.extension in ["scala", "java"]]
    srcjars = [f for f in ctx.rule.files.srcs if f.extension == "srcjar"]
    if not [f for f in sources if f.extension == "scala"] and not srcjars:
        return []
    options = ["-Wunused:all" if jars.scala3 else "-Wunused:_"] + list(toolchain.scalacopts) + list(getattr(attrs, "scalacopts", []))

    # rules_scala loads the semanticdb plugin itself; this compile does not, and
    # scalac rejects options for a plugin it has not loaded.
    options = [o for o in options if not o.startswith("-P:semanticdb:")]
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
        execution_requirements = JVM_CHECK_WORKER,
        resource_set = dx_heavy_resources,
        toolchain = None,
    )
    unused_deps = [unused_deps_map(target, ctx, attrs, [jars.library])] if analyzer and "dx-keep" not in attrs.tags else []
    return [JvmCheckInfo(log = log, tool = "scalac", unused_deps = unused_deps)]

def _scalac_check_impl(target, ctx):
    attrs = ctx.rule.attr
    if hasattr(attrs, "upstream"):
        upstream = single_target(attrs.upstream)
        if JvmCheckInfo not in upstream:
            return []
        return [upstream[JvmCheckInfo]]
    if ScalaInfo not in target:
        return []
    return _scalac_check(target, ctx, attrs)

scalac_check_aspect = aspect(
    implementation = _scalac_check_impl,
    doc = "Runs scalac (rules_scala, Scala 2.13 and 3.3+) without emitting classes over a target's sources and records the diagnostics without failing the build.",
    attr_aspects = ["upstream"],
    toolchains = [config_common.toolchain_type(SCALA_TOOLCHAIN_TYPE, mandatory = False)],
    attrs = {
        "_scalac_check": attr.label(
            default = "@dx_prebuilt//:scalac_check",
            executable = True,
            cfg = "exec",
        ),
        "_dependency_analyzer": attr.label(
            default = "@rules_scala//third_party/dependency_analyzer/src/main:dependency_analyzer",
            cfg = "exec",
        ),
    },
)
