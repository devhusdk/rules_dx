"""Pinned Elixir tree exposed through the rules_elixir ElixirInfo provider."""

load("@rules_elixir//private:elixir_build.bzl", "ElixirInfo")

def _dx_elixir_impl(ctx):
    tree = depset(ctx.files.srcs + [ctx.file.root])
    return [
        DefaultInfo(files = tree),
        ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo,
        ElixirInfo(
            release_dir = None,
            elixir_home = None,
            version_file = ctx.file.root,
            tree = tree,
        ),
    ]

dx_elixir = rule(
    implementation = _dx_elixir_impl,
    doc = "Exposes one in-tree Elixir installation as ElixirInfo on the resolved OTP.",
    attrs = {
        "root": attr.label(allow_single_file = True, mandatory = True, doc = "Version file at the Elixir root."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Every file of the Elixir tree."),
    },
    toolchains = ["@rules_erlang//tools:toolchain_type"],
)
