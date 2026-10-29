"""Cap'n Proto C++, Rust and Java code generation from the pinned source archives.

capnp and the C++ runtime come from the archive pinned in //capnproto:pins.bzl;
the Rust crates (capnp, capnpc) and capnpc-java with its Java runtime come from
the other archives pinned there. Every action reads only the declared import closure of CapnpInfo: the closure is
staged as symlinks under a private root and compiled with --no-standard-import,
so an import outside the declared deps fails with or without a sandbox.

C++ and Java are generated and compiled once per capnp_library by an aspect, so
two targets that share a schema library never link duplicate symbols.
Rust is one crate per capnp_library: the crate's dependencies mirror the library's
dependencies, and imports of a dependency's schemas become `::dep_crate::x_capnp`
paths, so a type is never duplicated across crates.
"""

load("@rules_cc//cc:find_cc_toolchain.bzl", "CC_TOOLCHAIN_ATTRS", "find_cpp_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_library")
load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "CapnpInfo")

CAPNP_GENERATED_SRCS_GROUP = "capnp_generated_srcs"

# Languages a capnp_codegen_shard can bind.
_LANGUAGES = ("cpp", "rust", "java")

CapnpCodegenInfo = provider(
    doc = "Which language a capnp codegen target generated.",
    fields = {"language": "string: cpp, rust or java."},
)

# Private attributes shared by every codegen rule, so a later toolchain swap
# touches one place. The compiler is the pinned source build, never the lint flag.
_PRIVATE_ATTRS = {
    "_capnp": attr.label(
        default = Label("@dx_capnproto//src/capnp:capnp_tool"),
        executable = True,
        cfg = "exec",
    ),
    "_capnpc_cxx": attr.label(
        default = Label("@dx_capnproto//src/capnp:capnpc-c++"),
        executable = True,
        cfg = "exec",
    ),
    "_capnpc_java": attr.label(
        default = Label("@dx_capnproto_java//:capnpc-java"),
        executable = True,
        cfg = "exec",
    ),
    "_capnpc_rust_dx": attr.label(
        default = Label("//capnproto/tools/capnpc_rust_dx"),
        executable = True,
        cfg = "exec",
    ),
}

def capnp_stage_closure(ctx, prefix, info):
    """Symlinks the declared import closure under a private root.

    Args:
      ctx: the rule or aspect context.
      prefix: package-relative directory to stage under; must be unique per target.
      info: the CapnpInfo of the library.

    Returns:
      A struct(root, files, staged): root is the exec path of the staging root, files
      the staged symlinks, and staged maps each import path to its symlink.
    """
    staged = {}
    sources = {}
    for entry in info.transitive_imports.to_list():
        if entry.path in sources:
            if sources[entry.path] != entry.file:
                fail("capnp codegen: two different files claim the import path " + entry.path + ": " + sources[entry.path].path + " and " + entry.file.path)
            continue
        sources[entry.path] = entry.file
        link = ctx.actions.declare_file(prefix + "/" + entry.path)
        ctx.actions.symlink(output = link, target_file = entry.file)
        staged[entry.path] = link
    pkg = ctx.label.package
    root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + prefix
    return struct(root = root, files = staged.values(), staged = staged)

CapnpCcAspectInfo = provider(
    doc = "C++ generated and compiled for one capnp_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated code of the closure.",
        "generated": "List[File]: headers and sources generated for this library's own schemas.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[CapnpInfo]
    name = target.label.name
    pkg = target.label.package
    stage = capnp_stage_closure(ctx, "_capnp_cc_stage/" + name, info)
    out_subdir = "_capnp_cc"
    out_root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + out_subdir
    headers = []
    sources = []
    args = ctx.actions.args()
    args.add("compile")
    args.add("--no-standard-import")
    args.add("-I", stage.root)
    args.add("--src-prefix", stage.root)
    args.add_all(["-o" + ctx.executable._capnpc_cxx.path + ":" + out_root])
    for src in info.direct_srcs:
        header = ctx.actions.declare_file(out_subdir + "/" + src.short_path + ".h")
        source = ctx.actions.declare_file(out_subdir + "/" + src.short_path + ".c++")
        headers.append(header)
        sources.append(source)
        args.add(stage.staged[src.short_path].path)
    if sources:
        ctx.actions.run(
            executable = ctx.executable._capnp,
            tools = [ctx.executable._capnpc_cxx],
            inputs = stage.files,
            outputs = headers + sources,
            arguments = [args],
            mnemonic = "CapnpCpp",
            progress_message = "capnp compile --c++ " + str(target.label),
        )
    runtime = ctx.attr._runtime[CcInfo]
    dep_infos = [dep[CapnpCcAspectInfo].cc_info for dep in ctx.rule.attr.deps]
    own_context = cc_common.create_compilation_context(
        headers = depset(headers),
        quote_includes = depset([out_root]),
        includes = depset([out_root]),
    )
    cc_toolchain = find_cpp_toolchain(ctx)
    feature_configuration = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    own_infos = []
    if sources:
        compilation_context, outputs = cc_common.compile(
            name = name + "_capnp_cc",
            actions = ctx.actions,
            feature_configuration = feature_configuration,
            cc_toolchain = cc_toolchain,
            srcs = sources,
            public_hdrs = headers,
            quote_includes = [out_root],
            includes = [out_root],
            compilation_contexts = [runtime.compilation_context] + [d.compilation_context for d in dep_infos],
            user_compile_flags = ["-std=c++17"],
        )
        linking_context, _ = cc_common.create_linking_context_from_compilation_outputs(
            name = name + "_capnp_cc",
            actions = ctx.actions,
            feature_configuration = feature_configuration,
            cc_toolchain = cc_toolchain,
            compilation_outputs = outputs,
            linking_contexts = [runtime.linking_context] + [d.linking_context for d in dep_infos],
        )
        own_infos.append(CcInfo(compilation_context = compilation_context, linking_context = linking_context))
    else:
        own_infos.append(CcInfo(compilation_context = own_context))
    return [CapnpCcAspectInfo(
        cc_info = cc_common.merge_cc_infos(cc_infos = own_infos + dep_infos),
        generated = headers + sources,
    )]

_cc_aspect = aspect(
    implementation = _cc_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(
        _PRIVATE_ATTRS,
        _runtime = attr.label(default = Label("@dx_capnproto//src/capnp:capnp_runtime"), providers = [CcInfo]),
    ) | CC_TOOLCHAIN_ATTRS,
    required_providers = [CapnpInfo],
    provides = [CapnpCcAspectInfo],
    fragments = ["cpp"],
    toolchains = use_cc_toolchain(),
)

def _cc_library_impl(ctx):
    aspect_infos = [dep[CapnpCcAspectInfo] for dep in ctx.attr.deps]
    own = [f for info in aspect_infos for f in info.generated]
    cc_info = cc_common.merge_cc_infos(
        cc_infos = [ctx.attr._runtime[CcInfo]] + [info.cc_info for info in aspect_infos],
    )
    return [
        DefaultInfo(files = depset(own)),
        cc_info,
        # Generated code owns no checked sources; it is not a cc_library to lint.
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{CAPNP_GENERATED_SRCS_GROUP: depset(own)}),
        CapnpCodegenInfo(language = "cpp"),
    ]

capnp_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ code and runtime for capnp_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [CapnpInfo],
            aspects = [_cc_aspect],
            doc = "capnp_library targets to generate C++ for.",
        ),
        "_runtime": attr.label(default = Label("@dx_capnproto//src/capnp:capnp_runtime"), providers = [CcInfo]),
    },
    provides = [CcInfo],
)

CapnpJavaAspectInfo = provider(
    doc = "Java sources and classes generated for one capnp_library closure.",
    fields = {
        "java_info": "JavaInfo of the compiled generated sources of the closure.",
        "srcjar": "File or None: srcjar of this library's own schemas.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _zip_entry(f):
    return f.tree_relative_path + "=" + f.path

def _java_aspect_impl(target, ctx):
    info = target[CapnpInfo]
    name = target.label.name
    dep_infos = [dep[CapnpJavaAspectInfo].java_info for dep in ctx.rule.attr.deps]
    runtime = ctx.attr._runtime[JavaInfo]
    if not info.direct_srcs:
        return [CapnpJavaAspectInfo(java_info = java_common.merge(dep_infos + [runtime]), srcjar = None)]
    stage = capnp_stage_closure(ctx, "_capnp_java_stage/" + name, info)
    tree = ctx.actions.declare_directory("_capnp_java/" + name + "_src")
    args = ctx.actions.args()
    args.add("compile")
    args.add("--no-standard-import")
    args.add("-I", stage.root)
    args.add("--src-prefix", stage.root)
    args.add("-o" + ctx.executable._capnpc_java.path + ":" + tree.path)
    for src in info.direct_srcs:
        args.add(stage.staged[src.short_path].path)
    ctx.actions.run(
        executable = ctx.executable._capnp,
        tools = [ctx.executable._capnpc_java],
        inputs = stage.files,
        outputs = [tree],
        arguments = [args],
        mnemonic = "CapnpJava",
        progress_message = "capnp compile --java " + str(target.label),
    )
    srcjar = ctx.actions.declare_file("_capnp_java/" + name + ".srcjar")
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = "CapnpJavaSrcjar",
        progress_message = "Packing " + srcjar.short_path,
    )
    jar = ctx.actions.declare_file("_capnp_java/" + name + ".jar")
    java_info = java_common.compile(
        ctx,
        source_jars = [srcjar],
        deps = dep_infos + [runtime],
        output = jar,
        java_toolchain = ctx.toolchains[_JAVA_TOOLCHAIN].java,
        javac_opts = ["-Xlint:none", "-nowarn"],
    )
    return [CapnpJavaAspectInfo(java_info = java_info, srcjar = srcjar)]

_java_aspect = aspect(
    implementation = _java_aspect_impl,
    attr_aspects = ["deps"],
    attrs = dict(_PRIVATE_ATTRS, **{
        "_runtime": attr.label(default = Label("//capnproto:runtime_java"), providers = [JavaInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    }),
    required_providers = [CapnpInfo],
    provides = [CapnpJavaAspectInfo],
    toolchains = [_JAVA_TOOLCHAIN],
    fragments = ["java"],
)

def _java_library_impl(ctx):
    aspect_infos = [dep[CapnpJavaAspectInfo] for dep in ctx.attr.deps]
    srcjars = [info.srcjar for info in aspect_infos if info.srcjar]
    java_info = java_common.merge([info.java_info for info in aspect_infos] + [ctx.attr._runtime[JavaInfo]])
    return [
        DefaultInfo(files = depset(srcjars)),
        java_info,
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{CAPNP_GENERATED_SRCS_GROUP: depset(srcjars)}),
        CapnpCodegenInfo(language = "java"),
    ]

capnp_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes and runtime for capnp_library schemas. Each library is generated and compiled once, by its own aspect. Every schema needs the $Java.package annotation from /capnp/java.capnp.",
    attrs = {
        "deps": attr.label_list(
            providers = [CapnpInfo],
            aspects = [_java_aspect],
            doc = "capnp_library targets to generate Java for.",
        ),
        "_runtime": attr.label(default = Label("//capnproto:runtime_java"), providers = [JavaInfo]),
    },
    provides = [JavaInfo],
)

CapnpRustCrateInfo = provider(
    doc = "The Rust crate generated for one capnp_library.",
    fields = {
        "crate_name": "string: the crate name dependents import.",
        "library": "string: label of the capnp_library the crate was generated from.",
        "schemas": "List[string]: import paths of the schemas the crate owns.",
    },
)

def _module_name(path):
    stem = path.rpartition("/")[2]
    if not stem.endswith(".capnp") or len(stem) == len(".capnp"):
        fail("capnp_rust_library: " + path + " is not a .capnp schema")
    return stem[:-len(".capnp")].replace("-", "_") + "_capnp"

def _rust_srcs_impl(ctx):
    info = ctx.attr.library[CapnpInfo]
    crates = {dep[CapnpRustCrateInfo].library: dep[CapnpRustCrateInfo] for dep in ctx.attr.crate_deps}
    for direct in info.direct_deps:
        if direct.label not in crates:
            fail(
                "capnp_rust_library " + display_label(ctx.label) + ": " + display_label(ctx.attr.library.label) +
                " depends on " + direct.label + ", which has no capnp_rust_library in deps. Rust sees only " +
                "direct dependencies, so add a capnp_rust_library of " + direct.label + " to deps",
            )
    if not info.direct_srcs:
        fail("capnp_rust_library " + display_label(ctx.label) + ": " + display_label(ctx.attr.library.label) + " owns no schemas")
    modules = {}
    for src in info.direct_srcs:
        module = _module_name(src.short_path)
        if module in modules.values():
            fail(
                "capnp_rust_library " + display_label(ctx.label) + ": two schemas of " + display_label(ctx.attr.library.label) +
                " generate module " + module + "; rename one or split the library",
            )
        modules[src.short_path] = module
    out_subdir = ctx.label.name + "_capnp_rs"
    generated = []
    for src in info.direct_srcs:
        dirname = src.short_path.rpartition("/")[0]
        generated.append(ctx.actions.declare_file(out_subdir + "/" + (dirname + "/" if dirname else "") + modules[src.short_path] + ".rs"))
    lib = ctx.actions.declare_file(out_subdir + "/lib.rs")
    stage = capnp_stage_closure(ctx, "_capnp_rs_stage/" + ctx.label.name, info)
    pkg = ctx.label.package
    out_dir = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + out_subdir
    args = ctx.actions.args()
    args.add("--capnp", ctx.executable._capnp)
    args.add("--root", stage.root)
    args.add("--out", out_dir)
    for src in info.direct_srcs:
        args.add("--own", src.short_path)
    for dep in crates.values():
        for path in dep.schemas:
            args.add("--provide", path + "=" + dep.crate_name)
    ctx.actions.run(
        executable = ctx.executable._capnpc_rust_dx,
        tools = [ctx.executable._capnp],
        inputs = stage.files,
        outputs = generated + [lib],
        arguments = [args],
        mnemonic = "CapnpRust",
        progress_message = "capnp compile --rust " + display_label(ctx.attr.library.label),
    )
    return [
        DefaultInfo(files = depset([lib] + generated)),
        # Only the generated *_capnp.rs files are bound; lib.rs is driver scaffolding.
        OutputGroupInfo(**{CAPNP_GENERATED_SRCS_GROUP: depset(generated)}),
        dx_quality_sources([], [], str(ctx.label)),
        CapnpCodegenInfo(language = "rust"),
        CapnpRustCrateInfo(
            crate_name = ctx.attr.crate_name,
            library = info.label,
            schemas = [src.short_path for src in info.direct_srcs],
        ),
    ]

_capnp_rust_srcs = rule(
    implementation = _rust_srcs_impl,
    doc = "Runs capnp and capnpc over one capnp_library and writes its crate sources.",
    attrs = {
        "crate_deps": attr.label_list(
            providers = [CapnpRustCrateInfo],
            doc = "NAME_generated targets of the capnp_rust_library of every direct capnp dep.",
        ),
        "crate_name": attr.string(mandatory = True),
        "library": attr.label(
            providers = [CapnpInfo],
            mandatory = True,
            doc = "capnp_library whose schemas the crate owns.",
        ),
    } | _PRIVATE_ATTRS,
)

def capnp_rust_library(name, library, deps = [], crate_name = None, **kwargs):
    """Rust crate for one capnp_library.

    The crate's deps must mirror the library's direct deps: one capnp_rust_library
    per capnp_library the schemas import. Imports of a dependency's schemas become
    ::dep_crate::x_capnp paths. /capnp/schema.capnp maps to the capnp crate. Point
    capnp_codegen_shard at NAME_generated.

    Args:
      name: Name of the rust_library.
      library: capnp_library whose schemas the crate owns.
      deps: capnp_rust_library targets of the library's direct capnp deps.
      crate_name: Crate name dependents import; defaults to name.
      **kwargs: Passed to the rust_library (visibility, tags, testonly).
    """
    generated = name + "_generated"
    crate = (crate_name or name).replace("-", "_")
    _capnp_rust_srcs(
        name = generated,
        library = library,
        crate_name = crate,
        crate_deps = [str(native.package_relative_label(dep)) + "_generated" for dep in deps],
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(
        name = name,
        srcs = [":" + generated],
        crate_name = crate,
        crate_root = None,
        edition = "2021",
        rustc_flags = ["--cap-lints=allow"],
        deps = [Label("//capnproto:runtime_rust")] + deps,
        **kwargs
    )

def _shard_impl(ctx):
    library = ctx.attr.library
    if ctx.attr.language in _LANGUAGES and CapnpCodegenInfo in library and library[CapnpCodegenInfo].language != ctx.attr.language:
        fail(
            "capnp_codegen_shard " + display_label(ctx.label) + ": language " + ctx.attr.language +
            " does not match " + display_label(library.label) + ", which generates " +
            library[CapnpCodegenInfo].language + ". Point library at a capnp_cc_library for cpp, " +
            "at NAME_generated of a capnp_rust_library for rust, or at a capnp_java_library for java",
        )
    if OutputGroupInfo not in library or CAPNP_GENERATED_SRCS_GROUP not in library[OutputGroupInfo]:
        fail(
            "capnp_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(library.label) + " has no " + CAPNP_GENERATED_SRCS_GROUP +
            " output group (want a capnp_cc_library, a capnp_java_library or a capnp_rust_library NAME_generated)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "capnp_codegen_shard",
        "capnp",
        display_label(library.label),
        CAPNP_GENERATED_SRCS_GROUP,
        library[OutputGroupInfo][CAPNP_GENERATED_SRCS_GROUP].to_list(),
    )

capnp_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the sources capnp generates. Every generated file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "cpp, rust or java.", mandatory = True),
        "library": attr.label(
            doc = "capnp_cc_library, capnp_java_library, or the NAME_generated target of a capnp_rust_library, whose generated files the entries bind.",
            mandatory = True,
        ),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
