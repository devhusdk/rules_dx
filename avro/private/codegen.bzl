"""Avro C++ and Java code generation from avro_library schemas."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//core/codegen:codegen.bzl", "codegen_shard_rule", "codegen_srcjar")
load("//core:defs.bzl", "display_label")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "AvroInfo")

AVRO_GENERATED_SRCS_GROUP = "avro_generated_srcs"

def _gen_args(ctx, schema, header):
    args = ctx.actions.args()
    args.add("-i", schema)
    args.add("-o", header)
    if ctx.attr.namespace:
        args.add("-n", ctx.attr.namespace)
    return args

def _cc_library_impl(ctx):
    pkg = ctx.label.package
    out_root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + "_avro_cc/" + ctx.label.name
    schemas = depset(transitive = [dep[AvroInfo].transitive_srcs for dep in ctx.attr.deps]).to_list()
    headers = []
    for schema in schemas:
        stem = schema.short_path.rsplit(".", 1)[0]
        if schema.extension == "avsc":
            header = ctx.actions.declare_file("_avro_cc/%s/%s.hh" % (ctx.label.name, stem))
            headers.append(header)
            ctx.actions.run(
                executable = ctx.executable._avrogencpp,
                inputs = [schema],
                outputs = [header],
                arguments = [_gen_args(ctx, schema.path, header.path)],
                mnemonic = "AvroCpp",
                progress_message = "avrogencpp " + schema.short_path,
            )
        else:
            union = ctx.actions.declare_file("_avro_cc/%s/%s.avsc" % (ctx.label.name, stem))
            header = ctx.actions.declare_file("_avro_cc/%s/%s.hh" % (ctx.label.name, stem))
            headers.append(header)
            ctx.actions.run(
                executable = ctx.executable._avdl_union,
                inputs = [schema],
                outputs = [union],
                arguments = [schema.path, union.path],
                mnemonic = "AvroIdlUnion",
                progress_message = "avdl union " + schema.short_path,
            )
            ctx.actions.run(
                executable = ctx.executable._avrogencpp,
                inputs = [union],
                outputs = [header],
                arguments = [_gen_args(ctx, union.path, header.path)],
                mnemonic = "AvroCpp",
                progress_message = "avrogencpp " + schema.short_path,
            )
    generated = headers
    runtime = ctx.attr._runtime[CcInfo]
    compilation_context = cc_common.create_compilation_context(
        headers = depset(generated),
        includes = depset([out_root]),
        quote_includes = depset([out_root]),
    )
    cc_info = cc_common.merge_cc_infos(cc_infos = [runtime, CcInfo(compilation_context = compilation_context)])
    return [
        DefaultInfo(files = depset(generated)),
        cc_info,
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{AVRO_GENERATED_SRCS_GROUP: depset(headers)}),
    ]

avro_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ headers and runtime for avro_library schemas.",
    attrs = {
        "deps": attr.label_list(providers = [AvroInfo], doc = "avro_library targets to generate C++ for."),
        "namespace": attr.string(doc = "C++ namespace passed to avrogencpp."),
        "_avdl_union": attr.label(default = Label("//avro/private/tools/avdl_union"), executable = True, cfg = "exec"),
        "_avrogencpp": attr.label(default = Label("@avro-cpp//:avrogencpp"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("@avro-cpp//:avro_cpp"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_library_impl(ctx):
    schemas = depset(transitive = [dep[AvroInfo].transitive_srcs for dep in ctx.attr.deps]).to_list()
    if not schemas:
        fail("avro_java_library " + display_label(ctx.label) + ": deps own no schemas")
    tree = ctx.actions.declare_directory(ctx.label.name + "_avro_java")
    inputs = []
    for schema in schemas:
        if schema.extension == "avsc":
            inputs.append(schema)
            continue
        union = ctx.actions.declare_file("_avro_java/%s/%s.avsc" % (ctx.label.name, schema.short_path.rsplit(".", 1)[0]))
        inputs.append(union)
        ctx.actions.run(
            executable = ctx.executable._avdl_union,
            inputs = [schema],
            outputs = [union],
            arguments = [schema.path, union.path],
            mnemonic = "AvroIdlUnion",
            progress_message = "avdl union " + schema.short_path,
        )
    args = ctx.actions.args()
    args.add_all(["compile", "schema"])
    args.add_all(inputs)
    args.add(tree.path)
    ctx.actions.run(
        executable = ctx.executable._avro_tools,
        inputs = inputs,
        outputs = [tree],
        arguments = [args],
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
        OutputGroupInfo(**{AVRO_GENERATED_SRCS_GROUP: depset([srcjar])}),
    ]

avro_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes for the schemas of avro_library deps and their imports, compiled once.",
    attrs = {
        "deps": attr.label_list(providers = [AvroInfo], doc = "avro_library targets to generate Java for."),
        "_avdl_union": attr.label(default = Label("//avro/private/tools/avdl_union"), executable = True, cfg = "exec"),
        "_avro_tools": attr.label(default = Label("//config:tool_avro"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("//core/quality/tools/jvm:avro_tools_import"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    },
    provides = [JavaInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

avro_codegen_shard = codegen_shard_rule(
    "avro_codegen_shard",
    "avro",
    AVRO_GENERATED_SRCS_GROUP,
    "want an avro_cc_library or avro_java_library",
    "cpp or java.",
    "Declares codegen plan entries for the headers and srcjars avro generates.",
    "avro_cc_library or avro_java_library whose outputs the entries bind.",
)
