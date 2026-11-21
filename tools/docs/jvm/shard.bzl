"""The JVM API reference shard action shared by the rules and the docs aspect."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core:rlocation.bzl", "repo_relative_path")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "srcs_manifest_line")

JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"
KOTLIN_TOOLCHAIN = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

JVM_LANGUAGES = {
    "java": struct(extension = "java", mnemonic = "DxDocsJavadoc"),
    "kotlin": struct(extension = "kt", mnemonic = "DxDocsDokka"),
    "scala": struct(extension = "scala", mnemonic = "DxDocsScaladoc"),
}

def _path(file):
    return file.path

def jvm_shard_action(ctx, language, java_info, own, package, dep_shards, stem, adapter_tool, convert_tool):
    """Runs the language adapter and the DocIr converter for one JVM library."""
    own_jars = []
    compiled = []
    for output in java_info.java_outputs:
        if output.compile_jar:
            own_jars.append(output.compile_jar.path)
            compiled.append(output.compile_jar)
        if output.class_jar:
            own_jars.append(output.class_jar.path)

    # The library's own jars sit in its transitive classpath; the adapter drops them by path.
    jar_sets = [java_info.transitive_compile_time_jars]
    if language == "kotlin":
        jar_sets.append(ctx.toolchains[KOTLIN_TOOLCHAIN].jvm_stdlibs.transitive_compile_time_jars)
    classpath = depset(transitive = jar_sets)

    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([
        srcs_manifest_line(f.path, repo_relative_path(f.short_path), not f.is_source)
        for f in own
    ])))
    classpath_file = ctx.actions.declare_file(stem + ".classpath.txt")
    classpath_lines = ctx.actions.args()
    classpath_lines.set_param_file_format("multiline")
    classpath_lines.add_all(classpath, map_each = _path, uniquify = True)
    ctx.actions.write(classpath_file, classpath_lines)
    own_jars_file = ctx.actions.declare_file(stem + ".own_jars.txt")
    ctx.actions.write(own_jars_file, "".join([path + "\n" for path in sorted(own_jars)]))

    api_json = ctx.actions.declare_file(stem + ".api.json")
    release = ctx.toolchains[JAVA_TOOLCHAIN].java.source_version
    adapter = ctx.actions.args()
    adapter.add("--srcs-manifest", manifest)
    adapter.add("--classpath-file", classpath_file)
    adapter.add("--own-jars-file", own_jars_file)
    adapter.add("--release", release)
    adapter.add("--out", api_json)
    ctx.actions.run(
        executable = adapter_tool,
        arguments = [adapter],
        inputs = depset(own + compiled + [manifest, classpath_file, own_jars_file], transitive = [classpath]),
        outputs = [api_json],
        mnemonic = JVM_LANGUAGES[language].mnemonic,
        toolchain = None,
        resource_set = dx_heavy_resources,
        progress_message = "Reading " + language + " API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    convert = ctx.actions.args()
    convert.add("jvm_convert")
    convert.add("--json", api_json)
    convert.add("--language", language)
    convert.add("--package", package)
    convert.add("--srcs-manifest", manifest)
    for dep_shard in dep_shards:
        convert.add("--dep-shard", dep_shard)
    convert.add("--out", shard)
    ctx.actions.run(
        executable = convert_tool,
        arguments = [convert],
        inputs = [api_json, manifest] + dep_shards,
        outputs = [shard],
        mnemonic = "DxDocsJvmConvert",
        toolchain = None,
        progress_message = "Converting " + language + " API docs for %{label}",
    )
    return struct(api_json = api_json, manifest = manifest, shard = shard)

def jvm_docs_aspect_result(target, ctx, language):
    """Returns the docs aspect output groups for a library written only in `language`."""
    if JavaInfo not in target or DocsShardInfo in target:
        return []
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return []
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or getattr(ctx.rule.attr, "plugins", []):
        return []
    extension = JVM_LANGUAGES[language].extension
    if not all([f.extension == extension for f in srcs]):
        return []
    if ctx.toolchains[JAVA_TOOLCHAIN] == None:
        return []
    adapter = getattr(ctx.executable, "_" + language + "_extract")
    action = jvm_shard_action(ctx, language, target[JavaInfo], srcs, ctx.label.name, [], "_dx_docs/" + ctx.label.name, adapter, ctx.executable._jvm_convert)
    return [OutputGroupInfo(dx_results = depset([action.shard]), dx_docs_shards = depset([action.shard]))]
