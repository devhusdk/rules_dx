"""FlatBuffers C++, Rust and Java code generation (flatc from the pinned source archive).

flatc and every runtime come from the one archive pinned in //flatbuffers:pins.bzl,
so the compiler and the runtimes cannot drift. Actions read only the declared
schema closure of FlatBuffersInfo.

C++ is generated per flatbuffers_library by an aspect. Rust is generated as one
crate per root schema (flatc --rust --gen-all), because flatc's Rust output uses
crate:: and super:: paths that only compile when the whole include closure sits
in one crate. Two Rust crates whose roots share a schema therefore each get their
own, non-interchangeable copy of its types.
Java is generated and compiled once per flatbuffers_library by an aspect, against
the Java runtime built from the same archive (Maven lags the flatc version).
"""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_library")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "FlatBuffersInfo", "resolve_root")

FLATBUFFERS_GENERATED_SRCS_GROUP = "flatbuffers_generated_srcs"

FlatBuffersCodegenInfo = provider(
    doc = "Which language a flatbuffers codegen target generated.",
    fields = {"language": "string: cpp, rust or java."},
)

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
        FlatBuffersCodegenInfo(language = "cpp"),
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

FlatBuffersJavaAspectInfo = provider(
    doc = "Java sources and classes generated for one flatbuffers_library closure.",
    fields = {
        "java_info": "JavaInfo of the compiled generated sources of the closure.",
        "srcjar": "File: srcjar of this library's own schemas.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _zip_entry(f):
    return f.tree_relative_path + "=" + f.path

def _java_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    name = target.label.name
    tree = ctx.actions.declare_directory("_fbs_java/" + name + "_src")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        inputs = info.transitive_srcs,
        outputs = [tree],
        arguments = ["--java", "-I", ".", "-o", tree.path] + [src.path for src in info.direct_srcs],
        mnemonic = "FlatcJava",
        progress_message = "flatc --java " + str(target.label),
    )
    srcjar = ctx.actions.declare_file("_fbs_java/" + name + ".srcjar")
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = "FlatcJavaSrcjar",
        progress_message = "Packing " + srcjar.short_path,
    )
    dep_infos = [dep[FlatBuffersJavaAspectInfo].java_info for dep in ctx.rule.attr.deps]
    jar = ctx.actions.declare_file("_fbs_java/" + name + ".jar")
    java_info = java_common.compile(
        ctx,
        source_jars = [srcjar],
        deps = dep_infos + [ctx.attr._runtime[JavaInfo]],
        output = jar,
        java_toolchain = ctx.toolchains[_JAVA_TOOLCHAIN].java,
        javac_opts = ["-Xlint:none", "-nowarn"],
    )
    return [FlatBuffersJavaAspectInfo(java_info = java_info, srcjar = srcjar)]

_java_aspect = aspect(
    implementation = _java_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(_PRIVATE_ATTRS, **{
        "_runtime": attr.label(default = Label("//flatbuffers:runtime_java"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    }),
    required_providers = [FlatBuffersInfo],
    provides = [FlatBuffersJavaAspectInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

def _java_library_impl(ctx):
    aspect_infos = [dep[FlatBuffersJavaAspectInfo] for dep in ctx.attr.deps]
    srcjars = [info.srcjar for info in aspect_infos]
    java_info = java_common.merge([info.java_info for info in aspect_infos] + [ctx.attr._runtime[JavaInfo]])
    return [
        DefaultInfo(files = depset(srcjars)),
        java_info,
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{FLATBUFFERS_GENERATED_SRCS_GROUP: depset(srcjars)}),
        FlatBuffersCodegenInfo(language = "java"),
    ]

flatbuffers_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes and runtime for flatbuffers_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [FlatBuffersInfo],
            aspects = [_java_aspect],
            doc = "flatbuffers_library targets to generate Java for.",
        ),
        "_runtime": attr.label(default = Label("//flatbuffers:runtime_java"), providers = [JavaInfo]),
    },
    provides = [JavaInfo],
)

def _rust_srcs_impl(ctx):
    info = ctx.attr.library[FlatBuffersInfo]
    root = resolve_root("flatbuffers_rust_library", ctx.label, ctx.attr.library, ctx.file.root)
    base = root.basename[:-len("." + root.extension)]
    generated = ctx.actions.declare_file(ctx.label.name + "_fbs_rs/" + base + "_generated.rs")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        inputs = info.transitive_srcs,
        outputs = [generated],
        arguments = _flatc_args(["--rust", "--gen-all"], False, generated.dirname, root),
        mnemonic = "FlatcRust",
        progress_message = "flatc --rust " + root.short_path,
    )
    lib = ctx.actions.declare_file(ctx.label.name + "_fbs_rs/lib.rs")
    ctx.actions.write(lib, "pub mod {0}_generated;\npub use {0}_generated::*;\n".format(base))
    return [
        DefaultInfo(files = depset([lib, generated])),
        OutputGroupInfo(**{FLATBUFFERS_GENERATED_SRCS_GROUP: depset([generated])}),
        dx_quality_sources([], [], str(ctx.label)),
        FlatBuffersCodegenInfo(language = "rust"),
    ]

_flatbuffers_rust_srcs = rule(
    implementation = _rust_srcs_impl,
    doc = "Runs flatc --rust --gen-all on one root schema and writes the crate root.",
    attrs = {
        "library": attr.label(
            providers = [FlatBuffersInfo],
            mandatory = True,
            doc = "flatbuffers_library owning the root schema.",
        ),
        "root": attr.label(
            allow_single_file = [".fbs"],
            doc = "Root schema, one of the library's srcs. Optional when the library owns exactly one.",
        ),
    } | _PRIVATE_ATTRS,
)

def flatbuffers_rust_library(name, library, root = None, **kwargs):
    """Rust crate for one root schema and its include closure.

    flatc --rust --gen-all emits the whole closure into one file, so the crate
    is per root, not per flatbuffers_library. The runtime comes from the pinned
    flatbuffers archive. Point flatbuffers_codegen_shard at NAME_generated.

    Args:
      name: Name of the rust_library.
      library: flatbuffers_library that owns the root schema.
      root: Root schema label; optional when the library owns one schema.
      **kwargs: Passed to the rust_library (visibility, tags, testonly).
    """
    generated = name + "_generated"
    _flatbuffers_rust_srcs(
        name = generated,
        library = library,
        root = root,
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(
        name = name,
        srcs = [":" + generated],
        crate_root = None,
        edition = "2021",
        rustc_flags = ["--cap-lints=allow"],
        deps = [Label("//flatbuffers:runtime_rust")],
        **kwargs
    )

def _shard_impl(ctx):
    library = ctx.attr.library
    if FlatBuffersCodegenInfo in library and ctx.attr.language in ("cpp", "rust", "java") and library[FlatBuffersCodegenInfo].language != ctx.attr.language:
        fail(
            "flatbuffers_codegen_shard " + display_label(ctx.label) + ": language " + ctx.attr.language +
            " does not match " + display_label(library.label) + ", which generates " +
            library[FlatBuffersCodegenInfo].language + ". Point library at a flatbuffers_cc_library for cpp, " +
            "at NAME_generated of a flatbuffers_rust_library for rust, " +
            "or at a flatbuffers_java_library for java",
        )
    if OutputGroupInfo not in library or FLATBUFFERS_GENERATED_SRCS_GROUP not in library[OutputGroupInfo]:
        fail(
            "flatbuffers_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(library.label) + " has no " + FLATBUFFERS_GENERATED_SRCS_GROUP +
            " output group (want a flatbuffers_cc_library, a flatbuffers_java_library or a flatbuffers_rust_library NAME_generated)",
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
        "language": attr.string(doc = "cpp, rust or java.", mandatory = True),
        "library": attr.label(
            doc = "flatbuffers_cc_library, flatbuffers_java_library, or the NAME_generated target of a flatbuffers_rust_library, whose generated files the entries bind.",
            mandatory = True,
        ),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
