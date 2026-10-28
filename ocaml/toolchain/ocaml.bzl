"""OCaml toolchains over one installed conda-forge tree."""

load("@bazel_skylib//rules:common_settings.bzl", "string_setting")
load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_ocaml//toolchain:BUILD.bzl", "ocaml_toolchain_adapter")
load("@rules_ocaml//toolchain:profiles.bzl", "ocaml_toolchain_profile")
load(":pins.bzl", "OCAML_VERSION")

STDLIB = "lib/ocaml"

STUBLIBS = STDLIB + "/stublibs"

TOOLS = {
    "compiler": "bin/ocamlc.opt",
    "native_compiler": "bin/ocamlopt.opt",
    "ocamllex": "bin/ocamllex.opt",
    "ocamlyacc": "bin/ocamlyacc",
    "vmruntime": "bin/ocamlrun",
}

NATIVE_RUNTIMES = {
    "dbg_runtime": STDLIB + "/libasmrund.a",
    "default_runtime": STDLIB + "/libasmrun.a",
    "instrumented_runtime": STDLIB + "/libasmruni.a",
    "pic_runtime": STDLIB + "/libasmrun_pic.a",
    "shared_runtime": STDLIB + "/libasmrun_shared.so",
    "std_runtime": STDLIB + "/libasmrun.a",
}

COMPILERS = {
    "sys": TOOLS["native_compiler"],
    "vm": TOOLS["compiler"],
}

def toolchain_fields(info, extra):
    """Returns the fields of one ToolchainInfo with the extra fields added."""
    fields = {key: getattr(info, key) for key in dir(info) if key not in ("to_json", "to_proto")}
    fields.update(extra)
    return fields

def native_cc_env(cc, assembler, linker, archiver, dist_bin):
    """Returns the CONDA_OCAML_* variables that point ocamlopt at the given C tools.

    The conda wrappers fall back to host as and cc when a variable is unset, so
    every tool ocamlopt runs is named here. The dist bin directory comes first on
    PATH so the wrappers themselves resolve; /usr/bin and /bin are the accepted
    host baseline (bash, dirname).
    """
    return {
        "CONDA_OCAML_AR": archiver,
        "CONDA_OCAML_AS": assembler + " -c",
        "CONDA_OCAML_CC": cc,
        "CONDA_OCAML_MKDLL": linker + " -shared",
        "CONDA_OCAML_MKEXE": linker,
        "PATH": dist_bin + ":/usr/bin:/bin",
    }

def _native_fields(ctx):
    cc = find_cc_toolchain(ctx)
    features = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    tool = lambda action: cc_common.get_tool_for_action(feature_configuration = features, action_name = action)
    return {
        "cc_files": depset(ctx.files.native_tools, transitive = [cc.all_files]),
        "cc_link_env_vars": native_cc_env(
            cc = tool(ACTION_NAMES.c_compile),
            assembler = tool(ACTION_NAMES.assemble),
            linker = tool(ACTION_NAMES.cpp_link_executable),
            archiver = tool(ACTION_NAMES.cpp_link_static_library),
            dist_bin = ctx.file.compiler.dirname,
        ),
        "cc_toolchain": "clang",
    }

def _dx_ocaml_toolchain_impl(ctx):
    upstream = ctx.attr.adapter[platform_common.ToolchainInfo]
    extra = {
        "cc_files": depset(),
        "cc_link_env_vars": {},
        "dllibs": ctx.files.dllibs,
        "dllibs_path": ctx.files.dllibs[0].dirname,
        "vmruntime": ctx.executable.vmruntime,
    }
    if ctx.attr.native:
        extra.update(_native_fields(ctx))
    return [platform_common.ToolchainInfo(**toolchain_fields(upstream, extra))]

dx_ocaml_toolchain = rule(
    implementation = _dx_ocaml_toolchain_impl,
    doc = "Adds the bytecode stub libraries and ocamlrun to one rules_ocaml toolchain.",
    attrs = {
        "adapter": attr.label(mandatory = True, providers = [platform_common.ToolchainInfo], doc = "The ocaml_toolchain_adapter target."),
        "compiler": attr.label(allow_single_file = True, mandatory = True, doc = "The compiler; its directory is the dist bin directory."),
        "dllibs": attr.label_list(allow_files = True, mandatory = True, doc = "Stub libraries ocamlrun loads."),
        "native_tools": attr.label_list(allow_files = True, doc = "The conda-ocaml-* wrapper scripts ocamlopt runs; they must sit beside each other in the action sandbox."),
        "native": attr.bool(default = False, doc = "Whether ocamlopt assembles and links with the C toolchain."),
        "vmruntime": attr.label(allow_single_file = True, executable = True, cfg = "exec", mandatory = True, doc = "The ocamlrun interpreter."),
    },
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)

def dx_ocaml_dist(name, emitters, licenses, version_file):
    """Declares the OCaml toolchains of one installed conda tree."""
    native.filegroup(name = name, srcs = [":licenses", ":version_file"])
    string_setting(name = "version", build_setting_default = OCAML_VERSION)
    native.filegroup(name = "stdlib", srcs = native.glob([STDLIB + "/**"]))
    native.filegroup(name = "stublibs", srcs = native.glob([STUBLIBS + "/*.so"]))
    native.filegroup(name = "runtime_libs", srcs = native.glob(["lib/*.so*"]))
    native.filegroup(
        name = "ocamlrun",
        srcs = [TOOLS["vmruntime"]],
        data = [":runtime_libs", ":stublibs", STDLIB + "/ld.conf"],
    )
    native.filegroup(name = "native_tools", srcs = native.glob(["bin/conda-ocaml-*"]))
    native.filegroup(name = "licenses", srcs = licenses)
    native.filegroup(name = "version_file", srcs = [version_file])
    for emitter in emitters:
        compiler = COMPILERS[emitter]
        ocaml_toolchain_adapter(
            name = emitter + "_adapter",
            host = "sys",
            target = emitter,
            compiler = compiler,
            sigcompiler = compiler,
            ocamllex = TOOLS["ocamllex"],
            ocamlyacc = TOOLS["ocamlyacc"],
            vmruntime = TOOLS["vmruntime"],
            stublibs = [":stublibs"],
            version = ":version",
            **(NATIVE_RUNTIMES if emitter == "sys" else {})
        )
        dx_ocaml_toolchain(
            name = emitter,
            adapter = ":" + emitter + "_adapter",
            compiler = compiler,
            dllibs = [":stublibs"],
            native = emitter == "sys",
            native_tools = [":native_tools"] if emitter == "sys" else [],
            vmruntime = TOOLS["vmruntime"],
        )
        ocaml_toolchain_profile(name = emitter + "_profile")
