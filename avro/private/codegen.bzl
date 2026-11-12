"""Avro C++ and Java code generation from avro_library schemas."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//core/codegen:codegen.bzl", "CODEGEN_LIBRARY_ATTRS", "codegen_library_providers", "codegen_srcjar")
load("//core:defs.bzl", "display_label")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "AvroInfo")

def _union(ctx, schemas, union):
    args = ctx.actions.args()
    args.add(union)
    args.add_all(schemas)
    ctx.actions.run(
        executable = ctx.executable._avro_union,
        inputs = schemas,
        outputs = [union],
        arguments = [args],
        mnemonic = "AvroUnion",
        progress_message = "avro union " + str(ctx.label),
    )

def _cc_library_impl(ctx):
    pkg = ctx.label.package
    out_root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + "_avro_cc/" + ctx.label.name
    schemas = depset(transitive = [dep[AvroInfo].transitive_srcs for dep in ctx.attr.deps]).to_list()
    if not schemas:
        fail("avro_cc_library " + display_label(ctx.label) + ": deps own no schemas")
    union = ctx.actions.declare_file("_avro_cc/%s/%s.avsc" % (ctx.label.name, ctx.label.name))
    header = ctx.actions.declare_file("_avro_cc/%s/%s%s.hh" % (ctx.label.name, pkg + "/" if pkg else "", ctx.label.name))
    _union(ctx, schemas, union)
    args = ctx.actions.args()
    args.add("-i", union)
    args.add("-o", header)
    if ctx.attr.namespace:
        args.add("-n", ctx.attr.namespace)
    ctx.actions.run(
        executable = ctx.executable._avrogencpp,
        inputs = [union],
        outputs = [header],
        arguments = [args],
        mnemonic = "AvroCpp",
        progress_message = "avrogencpp " + str(ctx.label),
    )
    headers = [header]
    runtime = ctx.attr._runtime[CcInfo]
    compilation_context = cc_common.create_compilation_context(
        headers = depset(headers),
        includes = depset([out_root]),
        quote_includes = depset([out_root]),
    )
    cc_info = cc_common.merge_cc_infos(cc_infos = [runtime, CcInfo(compilation_context = compilation_context)])
    return [
        DefaultInfo(files = depset(headers)),
        cc_info,
        dx_quality_sources([], [], str(ctx.label)),
    ] + codegen_library_providers(ctx, "avro", "cpp", header, ctx.attr.namespace)

avro_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "One C++ header with every type of the avro_library deps and their imports, and the runtime.",
    attrs = {
        "deps": attr.label_list(providers = [AvroInfo], doc = "avro_library targets to generate C++ for."),
        "namespace": attr.string(doc = "C++ namespace passed to avrogencpp."),
        "_avro_union": attr.label(default = Label("//avro/private/tools/avro_union"), executable = True, cfg = "exec"),
        "_avrogencpp": attr.label(default = Label("@dx_avro_cpp//:avrogencpp"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("@dx_avro_cpp//:avro_cpp"), providers = [CcInfo]),
    } | CODEGEN_LIBRARY_ATTRS,
    provides = [CcInfo],
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_library_impl(ctx):
    schemas = depset(transitive = [dep[AvroInfo].transitive_srcs for dep in ctx.attr.deps]).to_list()
    if not schemas:
        fail("avro_java_library " + display_label(ctx.label) + ": deps own no schemas")
    tree = ctx.actions.declare_directory(ctx.label.name + "_avro_java")
    union = ctx.actions.declare_file("_avro_java/%s/%s.avsc" % (ctx.label.name, ctx.label.name))
    _union(ctx, schemas, union)
    ctx.actions.run(
        executable = ctx.executable._avro_tools,
        inputs = [union],
        outputs = [tree],
        arguments = ["compile", "schema", union.path, tree.path],
        mnemonic = "AvroJava",
        progress_message = "avro-tools compile schema " + str(ctx.label),
    )
    srcjar = codegen_srcjar(ctx, tree, ctx.label.name + ".srcjar", "AvroJavaSrcjar")
    runtime = ctx.attr._runtime[JavaInfo]
    java_info = java_common.compile(
        ctx,
        source_jars = [srcjar],
        deps = [runtime],
        output = ctx.actions.declare_file(ctx.label.name + ".jar"),
        java_toolchain = ctx.toolchains[_JAVA_TOOLCHAIN].java,
        javac_opts = ["-Xlint:none", "-nowarn"],
    )
    return [
        DefaultInfo(files = depset([srcjar])),
        java_common.merge([java_info, runtime]),
        dx_quality_sources([], [], str(ctx.label)),
    ] + codegen_library_providers(ctx, "avro", "java", srcjar)

avro_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes for the schemas of avro_library deps and their imports, compiled once.",
    attrs = {
        "deps": attr.label_list(providers = [AvroInfo], doc = "avro_library targets to generate Java for."),
        "_avro_union": attr.label(default = Label("//avro/private/tools/avro_union"), executable = True, cfg = "exec"),
        "_avro_tools": attr.label(default = Label("//config:tool_avro"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("//core/quality/tools/jvm:avro_tools_import"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    } | CODEGEN_LIBRARY_ATTRS,
    provides = [JavaInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)
