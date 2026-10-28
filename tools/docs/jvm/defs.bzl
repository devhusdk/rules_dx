"""JVM API reference shards built from public sources by Javadoc and Dokka."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"
_KOTLIN_TOOLCHAIN = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

_JvmDirectSrcsInfo = provider(
    doc = "The sources one JVM library lists in its own srcs.",
    fields = {
        "plugins": "bool: whether the library runs annotation processors.",
        "srcs": "List[File]: the library's direct srcs.",
    },
)

def _direct_srcs_aspect_impl(_target, ctx):
    return [_JvmDirectSrcsInfo(
        srcs = list(getattr(ctx.rule.files, "srcs", [])),
        plugins = bool(getattr(ctx.rule.attr, "plugins", [])),
    )]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct srcs of a JVM library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def _where(ctx):
    return ctx.attr._language + "_doc_shard " + str(ctx.label)

def _shard_impl(ctx):
    language = ctx.attr._language
    lib = ctx.attr.lib
    info = lib[_JvmDirectSrcsInfo]
    extensions = ctx.attr._extensions
    own = sorted([f for f in info.srcs if f.extension in extensions], key = lambda f: f.path)
    shown = " or ".join(["." + e for e in extensions])
    if not own:
        fail(_where(ctx) + ": lib " + str(lib.label) + " lists no " + shown + " srcs")
    if len(own) != len(info.srcs):
        fail(_where(ctx) + ": lib " + str(lib.label) + " lists srcs that are not " + shown + " files; mixed sources and srcjars are not supported")
    if info.plugins:
        fail(_where(ctx) + ": lib " + str(lib.label) + " runs annotation processors or compiler plugins, which are not supported")
    package = ctx.attr.package or lib.label.name
    dep_shards = []
    for dep in ctx.attr.deps:
        shard_info = dep[DocsShardInfo]
        if shard_info.package == package:
            fail(_where(ctx) + ": dep " + str(dep.label) + " uses the same package '" + package + "'")
        dep_shards.append(shard_info.shard)

    java_info = lib[JavaInfo]
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
        for f in ctx.toolchains[_KOTLIN_TOOLCHAIN].jvm_stdlibs.transitive_compile_time_jars.to_list():
            classpath_jars[f.path] = f
    classpath = sorted([
        f
        for path, f in classpath_jars.items()
        if path not in own_jars
    ], key = lambda f: f.path)

    manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([
        srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source)
        for f in own
    ])))
    classpath_file = ctx.actions.declare_file(ctx.label.name + ".classpath.txt")
    ctx.actions.write(classpath_file, "".join([f.path + "\n" for f in classpath]))

    api_json = ctx.actions.declare_file(ctx.label.name + ".api.json")
    release = ctx.toolchains[_JAVA_TOOLCHAIN].java.source_version
    adapter = ctx.actions.args()
    adapter.add("--srcs-manifest", manifest)
    adapter.add("--classpath-file", classpath_file)
    adapter.add("--release", release)
    adapter.add("--out", api_json)
    ctx.actions.run(
        executable = ctx.executable._adapter,
        arguments = [adapter],
        inputs = own + classpath + compiled + [manifest, classpath_file],
        outputs = [api_json],
        mnemonic = ctx.attr._mnemonic,
        progress_message = "Reading " + language + " API docs for %{label}",
        use_default_shell_env = False,
    )

    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    convert = ctx.actions.args()
    convert.add("--json", api_json)
    convert.add("--language", language)
    convert.add("--package", package)
    convert.add("--srcs-manifest", manifest)
    for dep_shard in dep_shards:
        convert.add("--dep-shard", dep_shard)
    convert.add("--out", shard)
    ctx.actions.run(
        executable = ctx.executable._convert,
        arguments = [convert],
        inputs = [api_json, manifest] + dep_shards,
        outputs = [shard],
        mnemonic = "DxDocsJvmConvert",
        progress_message = "Converting " + language + " API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(
            api_json = depset([api_json]),
            srcs_manifest = depset([manifest]),
        ),
        DocsShardInfo(language = language, package = package, shard = shard),
    ]

def _jvm_shard_rule(language, adapter, mnemonic, extensions, toolchains = []):
    return rule(
        doc = "Converts one " + language + " library's public API into a DocIr shard.",
        implementation = _shard_impl,
        attrs = {
            "deps": attr.label_list(
                providers = [DocsShardInfo],
                doc = "Shards of libraries this one extends or links to.",
            ),
            "lib": attr.label(
                mandatory = True,
                providers = [JavaInfo],
                aspects = [_direct_srcs_aspect],
                doc = "The documented library; its direct srcs are documented.",
            ),
            "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
            "_adapter": attr.label(
                cfg = "exec",
                default = Label(adapter),
                executable = True,
            ),
            "_extensions": attr.string_list(default = extensions),
            "_convert": attr.label(
                cfg = "exec",
                default = Label("//tools/docs/jvm/convert:jvm_convert"),
                executable = True,
            ),
            "_language": attr.string(default = language),
            "_mnemonic": attr.string(default = mnemonic),
        },
        provides = [DocsShardInfo],
        toolchains = [_JAVA_TOOLCHAIN] + toolchains,
    )

docs_java_shard = _jvm_shard_rule(
    language = "java",
    adapter = "//tools/docs/jvm/javadoc:javadoc_extract",
    mnemonic = "DxDocsJavadoc",
    extensions = ["java"],
)

def java_doc_shard(name, lib, package = "", deps = [], **kwargs):
    """Declares the DocIr shard of one Java library.

    Args:
      name: Name of the shard target.
      lib: The documented java_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      deps: java_doc_shard targets of libraries whose types this one extends or links to.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_java_shard(
        name = name,
        lib = lib,
        package = package,
        deps = deps,
        **kwargs
    )

docs_kotlin_shard = _jvm_shard_rule(
    language = "kotlin",
    adapter = "//tools/docs/jvm/dokka:dokka_extract",
    mnemonic = "DxDocsDokka",
    extensions = ["kt"],
    toolchains = [_KOTLIN_TOOLCHAIN],
)

def kotlin_doc_shard(name, lib, package = "", deps = [], **kwargs):
    """Declares the DocIr shard of one Kotlin library.

    Args:
      name: Name of the shard target.
      lib: The documented kotlin_library; its srcs must all be .kt files.
      package: Package segment of every symbol id; defaults to the name of lib.
      deps: Doc shard targets of libraries whose types this one extends or links to.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_kotlin_shard(
        name = name,
        lib = lib,
        package = package,
        deps = deps,
        **kwargs
    )
