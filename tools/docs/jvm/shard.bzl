"""The JVM API reference shard action shared by the rules and the docs aspect."""

load("//core:resources.bzl", "dx_heavy_resources")
load("//tools/docs/ir:shard.bzl", "repo_relative", "srcs_manifest_line")

JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"
KOTLIN_TOOLCHAIN = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

JVM_LANGUAGES = {
    "java": struct(extension = "java", mnemonic = "DxDocsJavadoc"),
    "kotlin": struct(extension = "kt", mnemonic = "DxDocsDokka"),
    "scala": struct(extension = "scala", mnemonic = "DxDocsScaladoc"),
}

def jvm_shard_action(ctx, language, java_info, own, package, dep_shards, stem, adapter_tool, convert_tool):
    """Runs the language adapter and the DocIr converter for one JVM library.

    Args:
      ctx: The rule or aspect context.
      language: One of java, kotlin and scala.
      java_info: The JavaInfo of the documented library.
      own: The library's direct source files.
      package: Package segment of the symbol ids.
      dep_shards: DocIr shards of libraries this one links to.
      stem: Output name prefix, relative to the target's package.
      adapter_tool: The language's extraction executable.
      convert_tool: The DocIr converter executable.

    Returns:
      A struct with the DocIr `shard`, `api_json` and `manifest` files.
    """
    own_jars = {}
    compiled = []
    for output in java_info.java_outputs:
        if output.compile_jar:
            own_jars[output.compile_jar.path] = True
            compiled.append(output.compile_jar)
        if output.class_jar:
            own_jars[output.class_jar.path] = True

    # The adapter reads the sources alone. The library's own compile jar is only an input, so a
    # library that does not compile fails the build before extraction, in every language.
    classpath_jars = {f.path: f for f in java_info.transitive_compile_time_jars.to_list()}
    if language == "kotlin":
        # The Kotlin toolchain adds its stdlib to a library's compile classpath only privately.
        for f in ctx.toolchains[KOTLIN_TOOLCHAIN].jvm_stdlibs.transitive_compile_time_jars.to_list():
            classpath_jars[f.path] = f
    classpath = sorted([
        f
        for path, f in classpath_jars.items()
        if path not in own_jars
    ], key = lambda f: f.path)

    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([
        srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source)
        for f in own
    ])))
    classpath_file = ctx.actions.declare_file(stem + ".classpath.txt")
    ctx.actions.write(classpath_file, "".join([f.path + "\n" for f in classpath]))

    api_json = ctx.actions.declare_file(stem + ".api.json")
    release = ctx.toolchains[JAVA_TOOLCHAIN].java.source_version
    adapter = ctx.actions.args()
    adapter.add("--srcs-manifest", manifest)
    adapter.add("--classpath-file", classpath_file)
    adapter.add("--release", release)
    adapter.add("--out", api_json)
    ctx.actions.run(
        executable = adapter_tool,
        arguments = [adapter],
        inputs = own + classpath + compiled + [manifest, classpath_file],
        outputs = [api_json],
        mnemonic = JVM_LANGUAGES[language].mnemonic,
        toolchain = None,
        resource_set = dx_heavy_resources,
        progress_message = "Reading " + language + " API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    convert = ctx.actions.args()
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
