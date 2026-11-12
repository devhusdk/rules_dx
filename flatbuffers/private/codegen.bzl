"""FlatBuffers C++, Rust and Java code generation (flatc from the pinned source archive).

flatc and every runtime come from the one archive pinned in //flatbuffers:pins.bzl,
so the compiler and the runtimes cannot drift. Actions read only the staged closure.

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
load("//core/codegen:codegen.bzl", "codegen_shard_rule", "codegen_srcjar", "codegen_rust_library_kwargs")
load("//core:stage.bzl", "stage_closure")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "FlatBuffersInfo", "resolve_root")

FLATBUFFERS_GENERATED_SRCS_GROUP = "flatbuffers_generated_srcs"

FlatBuffersCodegenInfo = provider(
    doc = "Which language a flatbuffers codegen target generated.",
    fields = {"language": "string: cpp, rust or java."},
)

# The compiler is the pinned source build, never the lint flag.
_PRIVATE_ATTRS = {
    "_flatc": attr.label(
        default = Label("@dx_flatbuffers//:flatc"),
        executable = True,
        cfg = "exec",
    ),
}

def _flatc_args(language_flags, gen_object_api, out_dir, include_root, src_paths):
    args = ["-I", include_root] + language_flags
    if gen_object_api:
        args.append("--gen-object-api")
    return args + ["-o", out_dir] + src_paths

def _stage(ctx, prefix, info):
    entries = [struct(path = f.short_path, file = f) for f in info.transitive_srcs.to_list()]
    return stage_closure(ctx, prefix, entries, "flatbuffers")

FlatBuffersCcAspectInfo = provider(
    doc = "C++ headers generated for one flatbuffers_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated headers of the closure.",
        "headers": "List[File]: headers generated for this library's own schemas.",
        "stage_roots": "depset of the exec paths of the private include roots flatc reads.",
        "staged": "depset of the staged schema files flatc reads.",
    },
)

FlatBuffersCcStageInfo = provider(
    doc = "The private include roots and staged schemas the C++ flatc actions read, for tests.",
    fields = {
        "stage_roots": "depset of include root exec paths.",
        "staged": "depset of staged schema files.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    headers = []

    # Each flag variant gets its own root so variants do not declare the same path.
    subdir = "_fbs_obj" if ctx.attr.gen_object_api else "_fbs"
    stage = _stage(ctx, subdir + "_stage/" + target.label.name, info)
    for src in info.direct_srcs:
        out = ctx.actions.declare_file(subdir + "/" + src.short_path[:-len("." + src.extension)] + "_generated.h")
        ctx.actions.run(
            executable = ctx.executable._flatc,
            toolchain = None,
            inputs = stage.files,
            outputs = [out],
            arguments = _flatc_args(["--cpp", "--keep-prefix"], ctx.attr.gen_object_api, out.dirname, stage.root, [stage.staged[src.short_path].path]),
            mnemonic = "FlatcCpp",
            progress_message = "flatc --cpp " + src.short_path,
        )
        headers.append(out)
    own = CcInfo(compilation_context = cc_common.create_compilation_context(
        headers = depset(headers),
        quote_includes = depset([ctx.bin_dir.path + "/" + target.label.package + "/" + subdir if target.label.package else ctx.bin_dir.path + "/" + subdir]),
    ))
    deps = [dep[FlatBuffersCcAspectInfo] for dep in ctx.rule.attr.deps]
    return [FlatBuffersCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = [own] + [d.cc_info for d in deps]),
        headers = headers,
        stage_roots = depset([stage.root], transitive = [d.stage_roots for d in deps]),
        staged = depset(stage.files, transitive = [d.staged for d in deps]),
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
        FlatBuffersCcStageInfo(
            stage_roots = depset(transitive = [info.stage_roots for info in aspect_infos]),
            staged = depset(transitive = [info.staged for info in aspect_infos]),
        ),
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
        "stage_roots": "depset of the exec paths of the private include roots flatc reads.",
        "staged": "depset of the staged schema files flatc reads.",
    },
)

FlatBuffersJavaStageInfo = provider(
    doc = "The private include roots and staged schemas the Java flatc actions read, for tests.",
    fields = {
        "stage_roots": "depset of include root exec paths.",
        "staged": "depset of staged schema files.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_aspect_impl(target, ctx):
    info = target[FlatBuffersInfo]
    name = target.label.name
    stage = _stage(ctx, "_fbs_java_stage/" + name, info)
    tree = ctx.actions.declare_directory("_fbs_java/" + name + "_src")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        toolchain = None,
        inputs = stage.files,
        outputs = [tree],
        arguments = _flatc_args(["--java"], False, tree.path, stage.root, [stage.staged[src.short_path].path for src in info.direct_srcs]),
        mnemonic = "FlatcJava",
        progress_message = "flatc --java " + str(target.label),
    )
    srcjar = codegen_srcjar(ctx, tree, "_fbs_java/" + name + ".srcjar", "FlatcJavaSrcjar")
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
    deps = [dep[FlatBuffersJavaAspectInfo] for dep in ctx.rule.attr.deps]
    return [FlatBuffersJavaAspectInfo(
        java_info = java_info,
        srcjar = srcjar,
        stage_roots = depset([stage.root], transitive = [d.stage_roots for d in deps]),
        staged = depset(stage.files, transitive = [d.staged for d in deps]),
    )]

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
        FlatBuffersJavaStageInfo(
            stage_roots = depset(transitive = [info.stage_roots for info in aspect_infos]),
            staged = depset(transitive = [info.staged for info in aspect_infos]),
        ),
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
    stage = _stage(ctx, ctx.label.name + "_fbs_stage", info)
    generated = ctx.actions.declare_file(ctx.label.name + "_fbs_rs/" + base + "_generated.rs")
    ctx.actions.run(
        executable = ctx.executable._flatc,
        toolchain = None,
        inputs = stage.files,
        outputs = [generated],
        arguments = _flatc_args(["--rust", "--gen-all"], False, generated.dirname, stage.root, [stage.staged[root.short_path].path]),
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

def flatbuffers_rust_library(name, library, crate_name = None, deps = None, root = None, **kwargs):
    """Rust crate for one root schema and its include closure.

    flatc --rust --gen-all emits the whole closure into one file, so the crate
    is per root, not per flatbuffers_library. The runtime comes from the pinned
    flatbuffers archive. Point flatbuffers_codegen_shard at NAME_generated.

    Args:
      name: Name of the rust_library.
      library: flatbuffers_library that owns the root schema.
      crate_name: Crate name dependents import; defaults to name.
      deps: Extra Rust dependencies of the crate.
      root: Root schema label; optional when the library owns one schema.
      **kwargs: Passed to the rust_library (visibility, tags, testonly). edition, rustc_flags
        and the like replace the defaults; srcs and deps are added to them.
    """
    deps = deps or []
    generated = name + "_generated"
    _flatbuffers_rust_srcs(
        name = generated,
        library = library,
        root = root,
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(**codegen_rust_library_kwargs(name, ":" + generated, crate_name, [Label("//flatbuffers:runtime_rust")], deps, kwargs))

flatbuffers_codegen_shard = codegen_shard_rule(
    "flatbuffers_codegen_shard",
    "flatbuffers",
    FLATBUFFERS_GENERATED_SRCS_GROUP,
    "want a flatbuffers_cc_library, a flatbuffers_java_library or a flatbuffers_rust_library NAME_generated",
    "cpp, rust or java.",
    "Declares codegen plan entries for the sources flatc generates. Every generated file needs one entry with an EXEC_PATH suffix.",
    "flatbuffers_cc_library, flatbuffers_java_library, or the NAME_generated target of a flatbuffers_rust_library, whose generated files the entries bind.",
    codegen_info = FlatBuffersCodegenInfo,
    languages = ("cpp", "rust", "java"),
)
