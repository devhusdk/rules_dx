"""Avro C++ code generation with avrogencpp."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "AvroInfo")

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
    trees = []
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
            tree = ctx.actions.declare_directory("_avro_cc/%s/%s_idl" % (ctx.label.name, stem))
            trees.append(tree)
            ns = ["-n", ctx.attr.namespace] if ctx.attr.namespace else []
            ctx.actions.run_shell(
                tools = [ctx.executable._avrogencpp, ctx.executable._avro_tools],
                inputs = [schema],
                outputs = [tree],
                command = 'tmp=$(mktemp -d) && "$1" idl2schemata "$2" "$tmp" && for f in "$tmp"/*.avsc; do "$3" -i "$f" -o "$4/$(basename "${f%.avsc}").hh" "${@:5}"; done',
                arguments = [ctx.executable._avro_tools.path, schema.path, ctx.executable._avrogencpp.path, tree.path] + ns,
                mnemonic = "AvroIdlCpp",
                progress_message = "avro-tools idl2schemata " + schema.short_path,
            )
    generated = headers + trees
    runtime = ctx.attr._runtime[CcInfo]
    compilation_context = cc_common.create_compilation_context(
        headers = depset(generated),
        includes = depset([out_root] + [t.path for t in trees]),
        quote_includes = depset([out_root] + [t.path for t in trees]),
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
        "_avro_tools": attr.label(default = Label("//config:tool_avro"), executable = True, cfg = "exec"),
        "_avrogencpp": attr.label(default = Label("@avro-cpp//:avrogencpp"), executable = True, cfg = "exec"),
        "_runtime": attr.label(default = Label("@avro-cpp//:avro_cpp"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)

def _shard_impl(ctx):
    library = ctx.attr.library
    if OutputGroupInfo not in library or AVRO_GENERATED_SRCS_GROUP not in library[OutputGroupInfo]:
        fail(
            "avro_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(library.label) + " has no " + AVRO_GENERATED_SRCS_GROUP +
            " output group (want an avro_cc_library)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "avro_codegen_shard",
        "avro",
        display_label(library.label),
        AVRO_GENERATED_SRCS_GROUP,
        library[OutputGroupInfo][AVRO_GENERATED_SRCS_GROUP].to_list(),
    )

avro_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the headers avrogencpp generates from .avsc files.",
    attrs = {
        "entries": attr.string_list(doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per header.", mandatory = True),
        "language": attr.string(doc = "cpp.", mandatory = True),
        "library": attr.label(doc = "avro_cc_library whose headers the entries bind.", mandatory = True),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
