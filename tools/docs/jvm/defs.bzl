"""JVM API reference shards built from public sources by Javadoc, Dokka and Scaladoc."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "KOTLIN_TOOLCHAIN", "jvm_shard_action")

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

    action = jvm_shard_action(ctx, language, lib[JavaInfo], own, package, dep_shards, ctx.label.name, ctx.executable._adapter, ctx.executable._convert)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            api_json = depset([action.api_json]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = language, package = package, shard = action.shard),
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
        toolchains = [JAVA_TOOLCHAIN] + toolchains,
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
    toolchains = [KOTLIN_TOOLCHAIN],
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

docs_scala_shard = _jvm_shard_rule(
    language = "scala",
    adapter = "//tools/docs/jvm/scaladoc:scaladoc_extract",
    mnemonic = "DxDocsScaladoc",
    extensions = ["scala"],
)

def scala_doc_shard(name, lib, package = "", deps = [], **kwargs):
    """Declares the DocIr shard of one Scala 2.13 library.

    Args:
      name: Name of the shard target.
      lib: The documented scala_library; its srcs must all be .scala files.
      package: Package segment of every symbol id; defaults to the name of lib.
      deps: Doc shard targets of libraries whose types this one extends or links to.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_scala_shard(
        name = name,
        lib = lib,
        package = package,
        deps = deps,
        **kwargs
    )
