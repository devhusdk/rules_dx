"""FlatBuffers C++ code generation (flatc from the pinned source archive).

flatc and the C++ runtime come from the one archive pinned in //flatbuffers:pins.bzl,
so the compiler and the runtime cannot drift. Actions read only the declared
schema closure of FlatBuffersInfo.
"""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "FlatBuffersInfo")

FLATBUFFERS_GENERATED_SRCS_GROUP = "flatbuffers_generated_srcs"

# Private attributes shared by every codegen rule, so a later toolchain swap
# touches one place. The compiler is the pinned source build, never the lint flag.
_PRIVATE_ATTRS = {
    "_flatc": attr.label(
        default = Label("@dx_flatbuffers//:flatc"),
        executable = True,
        cfg = "exec",
    ),
}

def _flatc_args(language_flags, gen_object_api, out_dir, src):
    args = ["-I", "."] + language_flags
    if gen_object_api:
        args.append("--gen-object-api")
    return args + ["-o", out_dir, src.path]

FlatBuffersCcAspectInfo = provider(
    doc = "C++ headers generated for one flatbuffers_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated headers of the closure.",
        "headers": "List[File]: headers generated for this library's own schemas.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    headers = []

    # The flags change the output, so each variant gets its own root: one
    # flatbuffers_library reached by cc libraries with different gen_object_api
    # values would otherwise declare the same path from conflicting actions.
    subdir = "_fbs_obj" if ctx.attr.gen_object_api else "_fbs"
    for src in info.direct_srcs:
        out = ctx.actions.declare_file(subdir + "/" + src.short_path[:-len("." + src.extension)] + "_generated.h")
        ctx.actions.run(
            executable = ctx.executable._flatc,
            inputs = info.transitive_srcs,
            outputs = [out],
            arguments = _flatc_args(["--cpp", "--keep-prefix"], ctx.attr.gen_object_api, out.dirname, src),
            mnemonic = "FlatcCpp",
            progress_message = "flatc --cpp " + src.short_path,
        )
        headers.append(out)
    own = CcInfo(compilation_context = cc_common.create_compilation_context(
        headers = depset(headers),
        quote_includes = depset([ctx.bin_dir.path + "/" + target.label.package + "/" + subdir if target.label.package else ctx.bin_dir.path + "/" + subdir]),
    ))
    dep_infos = [dep[FlatBuffersCcAspectInfo].cc_info for dep in ctx.rule.attr.deps]
    return [FlatBuffersCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = [own] + dep_infos),
        headers = headers,
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(_PRIVATE_ATTRS, gen_object_api = attr.bool(default = False)),
    required_providers = [FlatBuffersInfo],
    provides = [FlatBuffersCcAspectInfo],
)

def _cc_library_impl(ctx):
    aspect_infos = [dep[FlatBuffersCcAspectInfo] for dep in ctx.attr.deps]
    own_headers = [h for info in aspect_infos for h in info.headers]
    cc_info = cc_common.merge_cc_infos(
        cc_infos = [ctx.attr._runtime[CcInfo]] + [info.cc_info for info in aspect_infos],
    )
    return [
        DefaultInfo(files = depset(own_headers)),
        cc_info,
        # Generated code owns no checked sources; it is not a cc_library to lint.
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{FLATBUFFERS_GENERATED_SRCS_GROUP: depset(own_headers)}),
    ]

flatbuffers_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ headers and runtime for flatbuffers_library schemas. Each library's headers are generated once, by its own aspect action.",
    attrs = {
        "deps": attr.label_list(
            providers = [FlatBuffersInfo],
            aspects = [_cc_aspect],
            doc = "flatbuffers_library targets to generate C++ for.",
        ),
        "gen_object_api": attr.bool(default = False, doc = "Also generate the object API (--gen-object-api)."),
        "_runtime": attr.label(default = Label("@dx_flatbuffers//:runtime_cc"), providers = [CcInfo]),
    } | _PRIVATE_ATTRS,
    provides = [CcInfo],
)

def _shard_impl(ctx):
    library = ctx.attr.library
    if OutputGroupInfo not in library or FLATBUFFERS_GENERATED_SRCS_GROUP not in library[OutputGroupInfo]:
        fail(
            "flatbuffers_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(library.label) + " has no " + FLATBUFFERS_GENERATED_SRCS_GROUP +
            " output group (want a flatbuffers_cc_library)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "flatbuffers_codegen_shard",
        "flatbuffers",
        display_label(library.label),
        FLATBUFFERS_GENERATED_SRCS_GROUP,
        library[OutputGroupInfo][FLATBUFFERS_GENERATED_SRCS_GROUP].to_list(),
    )

flatbuffers_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the sources flatc generates. Every generated file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "cpp.", mandatory = True),
        "library": attr.label(
            doc = "flatbuffers_cc_library whose generated headers the entries bind.",
            mandatory = True,
        ),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
