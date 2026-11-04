"""Rule that writes the selected C++ compiler identity to JSON."""

load("//cc/toolchains:bindings.bzl", "cc_toolchain_identity", "cc_toolchain_info", "cc_toolchain_toolchains")

def _cc_context_identity_impl(ctx):
    info = cc_toolchain_info(ctx)
    cc_info = info.cc if hasattr(info, "cc") else info
    identity = cc_toolchain_identity(ctx)
    out = ctx.actions.declare_file(ctx.label.name + ".cc_identity.json")
    ctx.actions.write(out, json.encode({
        "configuration": ctx.bin_dir.path.split("/")[1],
        "compiler": cc_info.compiler,
        "family": identity.family,
        "driver": identity.driver,
        "abi": identity.abi,
        "target": cc_info.target_gnu_system_name,
    }) + "\n")
    return [DefaultInfo(files = depset([out]))]

cc_context_identity = rule(
    implementation = _cc_context_identity_impl,
    doc = "Writes the selected C/C++ compiler identity as JSON.",
    toolchains = cc_toolchain_toolchains(),
)
