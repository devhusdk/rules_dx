"""Nix source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _nix_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("nix", ["nix"])], str(ctx.label)),
    ]

nix_library = rule(
    implementation = _nix_library_impl,
    doc = "Owns the Nix files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".nix"], doc = "Nix files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Nix libraries this package depends on."),
    },
)
