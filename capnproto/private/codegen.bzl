"""Cap'n Proto C++, Rust and Java code generation from the pinned source archives.

capnp and the C++ runtime come from the archive pinned in //capnproto:pins.bzl;
the Rust crates (capnp, capnpc) and capnpc-java with its Java runtime come from
the other archives pinned there. Every action reads only the declared import closure of CapnprotoInfo: the closure is
staged as symlinks under a private root and compiled with --no-standard-import,
so an import outside the declared deps fails with or without a sandbox.

C++ and Java are generated and compiled once per capnproto_library by an aspect, so
two targets that share a schema library never link duplicate symbols.
Rust is one crate per capnproto_library: the crate's dependencies mirror the library's
dependencies, and imports of a dependency's schemas become `::dep_crate::x_capnp`
paths, so a type is never duplicated across crates.
"""

load("@rules_cc//cc:find_cc_toolchain.bzl", "CC_TOOLCHAIN_ATTRS", "find_cpp_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", "rust_library")
load("//core/codegen:codegen.bzl", "CODEGEN_LIBRARY_ATTRS", "codegen_library_providers", "codegen_rust_library_kwargs", "codegen_srcjar")
load("//core:defs.bzl", "display_label")
load("//core:stage.bzl", "stage_closure")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "CapnprotoInfo")

# The compiler is the pinned source build, never the lint flag.
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
        default = Label("//capnproto/private/tools/capnpc_rust_dx"),
        executable = True,
        cfg = "exec",
    ),
}

CapnpCcAspectInfo = provider(
    doc = "C++ generated and compiled for one capnproto_library closure.",
    fields = {
        "cc_info": "CcInfo of the generated code of the closure.",
        "generated": "List[File]: headers and sources generated for this library's own schemas.",
    },
)

def _cc_aspect_impl(target, ctx):
    info = target[CapnprotoInfo]
    name = target.label.name
    pkg = target.label.package
    stage = stage_closure(ctx, "_capnp_cc_stage/" + name, info.transitive_imports.to_list(), "capnp")
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
            toolchain = None,
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
    required_providers = [CapnprotoInfo],
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
    ] + codegen_library_providers(ctx, "capnp", "cpp", own)

capnproto_cc_library = rule(
    implementation = _cc_library_impl,
    doc = "C++ code and runtime for capnproto_library schemas. Each library is generated and compiled once, by its own aspect.",
    attrs = {
        "deps": attr.label_list(
            providers = [CapnprotoInfo],
            aspects = [_cc_aspect],
            doc = "capnproto_library targets to generate C++ for.",
        ),
        "_runtime": attr.label(default = Label("@dx_capnproto//src/capnp:capnp_runtime"), providers = [CcInfo]),
    } | CODEGEN_LIBRARY_ATTRS,
    provides = [CcInfo],
)

CapnpJavaAspectInfo = provider(
    doc = "Java sources and classes generated for one capnproto_library closure.",
    fields = {
        "java_info": "JavaInfo of the compiled generated sources of the closure.",
        "srcjar": "File or None: srcjar of this library's own schemas.",
    },
)

_JAVA_TOOLCHAIN = "@bazel_tools//tools/jdk:toolchain_type"

def _java_aspect_impl(target, ctx):
    info = target[CapnprotoInfo]
    name = target.label.name
    dep_infos = [dep[CapnpJavaAspectInfo].java_info for dep in ctx.rule.attr.deps]
    runtime = ctx.attr._runtime[JavaInfo]
    if not info.direct_srcs:
        return [CapnpJavaAspectInfo(java_info = java_common.merge(dep_infos + [runtime]), srcjar = None)]
    stage = stage_closure(ctx, "_capnp_java_stage/" + name, info.transitive_imports.to_list(), "capnp")
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
        toolchain = None,
        tools = [ctx.executable._capnpc_java],
        inputs = stage.files,
        outputs = [tree],
        arguments = [args],
        mnemonic = "CapnpJava",
        progress_message = "capnp compile --java " + str(target.label),
    )
    srcjar = codegen_srcjar(ctx, tree, "_capnp_java/" + name + ".srcjar", "CapnpJavaSrcjar")
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
    required_providers = [CapnprotoInfo],
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
    ] + codegen_library_providers(ctx, "capnp", "java", srcjars)

capnproto_java_library = rule(
    implementation = _java_library_impl,
    doc = "Java classes and runtime for capnproto_library schemas. Each library is generated and compiled once, by its own aspect. Every schema needs the $Java.package annotation from /capnp/java.capnp.",
    attrs = {
        "deps": attr.label_list(
            providers = [CapnprotoInfo],
            aspects = [_java_aspect],
            doc = "capnproto_library targets to generate Java for.",
        ),
        "_runtime": attr.label(default = Label("//capnproto:runtime_java"), providers = [JavaInfo]),
    } | CODEGEN_LIBRARY_ATTRS,
    provides = [JavaInfo],
)

CapnpRustCrateInfo = provider(
    doc = "The Rust crate generated for one capnproto_library.",
    fields = {
        "crate_name": "string: the crate name dependents import.",
        "library": "string: label of the capnproto_library the crate was generated from.",
        "schemas": "List[string]: import paths of the schemas the crate owns.",
    },
)

def _module_name(path):
    stem = path.rpartition("/")[2]
    if not stem.endswith(".capnp") or len(stem) == len(".capnp"):
        fail("capnproto_rust_library: " + path + " is not a .capnp schema")
    return stem[:-len(".capnp")].replace("-", "_") + "_capnp"

def _rust_srcs_impl(ctx):
    info = ctx.attr.library[CapnprotoInfo]
    crates = {dep[CapnpRustCrateInfo].library: dep[CapnpRustCrateInfo] for dep in ctx.attr.crate_deps}
    for direct in info.direct_deps:
        if direct.label not in crates:
            fail(
                "capnproto_rust_library " + display_label(ctx.label) + ": " + display_label(ctx.attr.library.label) +
                " depends on " + direct.label + ", which has no capnproto_rust_library in deps. Rust sees only " +
                "direct dependencies, so add a capnproto_rust_library of " + direct.label + " to deps",
            )
    if not info.direct_srcs:
        fail("capnproto_rust_library " + display_label(ctx.label) + ": " + display_label(ctx.attr.library.label) + " owns no schemas")
    modules = {}
    for src in info.direct_srcs:
        module = _module_name(src.short_path)
        if module in modules.values():
            fail(
                "capnproto_rust_library " + display_label(ctx.label) + ": two schemas of " + display_label(ctx.attr.library.label) +
                " generate module " + module + "; rename one or split the library",
            )
        modules[src.short_path] = module
    out_subdir = ctx.label.name + "_capnp_rs"
    generated = []
    for src in info.direct_srcs:
        dirname = src.short_path.rpartition("/")[0]
        generated.append(ctx.actions.declare_file(out_subdir + "/" + (dirname + "/" if dirname else "") + modules[src.short_path] + ".rs"))
    lib = ctx.actions.declare_file(out_subdir + "/lib.rs")
    stage = stage_closure(ctx, "_capnp_rs_stage/" + ctx.label.name, info.transitive_imports.to_list(), "capnp")
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
        toolchain = None,
        tools = [ctx.executable._capnp],
        inputs = stage.files,
        outputs = generated + [lib],
        arguments = [args],
        mnemonic = "CapnpRust",
        progress_message = "capnp compile --rust " + display_label(ctx.attr.library.label),
    )
    return [
        DefaultInfo(files = depset([lib] + generated)),
        dx_quality_sources([], [], str(ctx.label)),
        CapnpRustCrateInfo(
            crate_name = ctx.attr.crate_name,
            library = info.label,
            schemas = [src.short_path for src in info.direct_srcs],
        ),
    ] + codegen_library_providers(ctx, "capnp", "rust", generated)

_capnp_rust_srcs = rule(
    implementation = _rust_srcs_impl,
    doc = "Runs capnp and capnpc over one capnproto_library and writes its crate sources.",
    attrs = {
        "crate_deps": attr.label_list(
            providers = [CapnpRustCrateInfo],
            doc = "NAME_generated targets of the capnproto_rust_library of every direct capnp dep.",
        ),
        "crate_name": attr.string(mandatory = True),
        "library": attr.label(
            providers = [CapnprotoInfo],
            mandatory = True,
            doc = "capnproto_library whose schemas the crate owns.",
        ),
    } | _PRIVATE_ATTRS | CODEGEN_LIBRARY_ATTRS,
)

def capnproto_rust_library(name, library, crate_name = None, deps = None, codegen_path = None, **kwargs):
    """Rust crate for one capnproto_library.

    The crate's deps must mirror the library's direct deps: one capnproto_rust_library
    per capnproto_library the schemas import. Imports of a dependency's schemas become
    ::dep_crate::x_capnp paths. /capnp/schema.capnp maps to the capnp crate.

    Args:
      name: Name of the rust_library.
      library: capnproto_library whose schemas the crate owns.
      crate_name: Crate name dependents import; defaults to name.
      deps: capnproto_rust_library targets of the library's direct capnp deps.
      codegen_path: Logical directory of the generated files in the dx sync projection.
      **kwargs: Passed to the rust_library (visibility, tags, testonly). edition, rustc_flags
        and the like replace the defaults; srcs and deps are added to them.
    """
    deps = deps or []
    generated = name + "_generated"
    crate = (crate_name or name).replace("-", "_")
    _capnp_rust_srcs(
        name = generated,
        library = library,
        crate_name = crate,
        crate_deps = [str(native.package_relative_label(dep)) + "_generated" for dep in deps],
        codegen_path = codegen_path,
        tags = kwargs.get("tags"),
        testonly = kwargs.get("testonly"),
        visibility = kwargs.get("visibility"),
    )
    rust_library(**codegen_rust_library_kwargs(name, ":" + generated, crate, [Label("//capnproto:runtime_rust")], deps, kwargs))
