"""OCaml toolchains over one installed conda-forge tree."""

load("@bazel_skylib//rules:common_settings.bzl", "string_setting")
load("@rules_ocaml//toolchain:BUILD.bzl", "ocaml_toolchain_adapter")
load("@rules_ocaml//toolchain:profiles.bzl", "ocaml_toolchain_profile")
load(":pins.bzl", "OCAML_VERSION")

STDLIB = "lib/ocaml"

STUBLIBS = STDLIB + "/stublibs"

TOOLS = {
    "compiler": "bin/ocamlc.opt",
    "ocamllex": "bin/ocamllex.opt",
    "ocamlyacc": "bin/ocamlyacc",
    "vmruntime": "bin/ocamlrun",
}

def toolchain_fields(info, extra):
    """Returns the fields of one ToolchainInfo with the extra fields added."""
    fields = {key: getattr(info, key) for key in dir(info) if key not in ("to_json", "to_proto")}
    fields.update(extra)
    return fields

def _dx_ocaml_toolchain_impl(ctx):
    upstream = ctx.attr.adapter[platform_common.ToolchainInfo]
    return [platform_common.ToolchainInfo(**toolchain_fields(upstream, {
        "dllibs": ctx.files.dllibs,
        "dllibs_path": ctx.files.dllibs[0].dirname,
        "vmruntime": ctx.executable.vmruntime,
    }))]

dx_ocaml_toolchain = rule(
    implementation = _dx_ocaml_toolchain_impl,
    doc = "Adds the bytecode stub libraries and ocamlrun to one rules_ocaml toolchain.",
    attrs = {
        "adapter": attr.label(mandatory = True, providers = [platform_common.ToolchainInfo], doc = "The ocaml_toolchain_adapter target."),
        "dllibs": attr.label_list(allow_files = True, mandatory = True, doc = "Stub libraries ocamlrun loads."),
        "vmruntime": attr.label(allow_single_file = True, executable = True, cfg = "exec", mandatory = True, doc = "The ocamlrun interpreter."),
    },
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
    native.filegroup(name = "licenses", srcs = licenses)
    native.filegroup(name = "version_file", srcs = [version_file])
    for emitter in emitters:
        ocaml_toolchain_adapter(
            name = emitter + "_adapter",
            host = "sys",
            target = emitter,
            compiler = TOOLS["compiler"],
            sigcompiler = TOOLS["compiler"],
            ocamllex = TOOLS["ocamllex"],
            ocamlyacc = TOOLS["ocamlyacc"],
            vmruntime = TOOLS["vmruntime"],
            stublibs = [":stublibs"],
            version = ":version",
        )
        dx_ocaml_toolchain(
            name = emitter,
            adapter = ":" + emitter + "_adapter",
            dllibs = [":stublibs"],
            vmruntime = TOOLS["vmruntime"],
        )
        ocaml_toolchain_profile(name = emitter + "_profile")
