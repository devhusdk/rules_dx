"""Rule that writes the selected C++ compiler identity to JSON."""

load("//cc/private/toolchains:bindings.bzl", "cc_toolchain_identity", "cc_toolchain_info", "cc_toolchain_toolchains")

def _cc_context_identity_impl(ctx):
    cc_info = cc_toolchain_info(ctx)
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

def _cc_context_exec_identity_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".cc_identity.json")
    ctx.actions.symlink(output = out, target_file = ctx.file.identity)
    return [DefaultInfo(files = depset([out]))]

cc_context_exec_identity = rule(
    implementation = _cc_context_exec_identity_impl,
    doc = "Builds the compiler identity in the exec configuration.",
    attrs = {
        "identity": attr.label(
            doc = "The cc_context_identity target to build for exec.",
            cfg = "exec",
            allow_single_file = True,
            mandatory = True,
        ),
    },
)
