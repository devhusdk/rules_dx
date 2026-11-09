"""Avro C++ and Java code generation from avro_library schemas."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
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
        "_avdl_union": attr.label(default = Label("//avro/tools/avdl_union"), executable = True, cfg = "exec"),
        "_avrogencpp": attr.label(default = Label("@avro-cpp//:avrogencpp"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("@avro-cpp//:avro_cpp"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _zip_entry(f):
    return f.tree_relative_path + "=" + f.path

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
    srcjar = ctx.actions.declare_file(ctx.label.name + ".srcjar")
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        toolchain = None,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = "AvroJavaSrcjar",
        progress_message = "Packing " + srcjar.short_path,
    )
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
        "_avdl_union": attr.label(default = Label("//avro/tools/avdl_union"), executable = True, cfg = "exec"),
        "_avro_tools": attr.label(default = Label("//config:tool_avro"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("//quality/tools/jvm:avro_tools_import"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    },
    provides = [JavaInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

def _shard_impl(ctx):
    generated = ctx.attr.generated
    if OutputGroupInfo not in generated or AVRO_GENERATED_SRCS_GROUP not in generated[OutputGroupInfo]:
        fail(
            "avro_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(generated.label) + " has no " + AVRO_GENERATED_SRCS_GROUP +
            " output group (want an avro_cc_library or avro_java_library)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "avro_codegen_shard",
        "avro",
        display_label(generated.label),
        AVRO_GENERATED_SRCS_GROUP,
        generated[OutputGroupInfo][AVRO_GENERATED_SRCS_GROUP].to_list(),
    )

avro_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the headers and srcjars avro generates.",
    attrs = {
        "entries": attr.string_list(doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per file.", mandatory = True),
        "language": attr.string(doc = "cpp or java.", mandatory = True),
        "generated": attr.label(doc = "avro_cc_library or avro_java_library whose outputs the entries bind.", mandatory = True),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
