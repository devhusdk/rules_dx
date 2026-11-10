"""Make source ownership for dx quality."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources", "named_source_class_error")

def _make_library_impl(ctx):
    for f in ctx.files.srcs:
        error = named_source_class_error("make", f.basename)
        if error != "":
            fail("make_library " + str(ctx.label) + ": " + error)
    direct_sources = {}
    if len(ctx.files.srcs) > 0:
        direct_sources["make"] = depset(ctx.files.srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

make_library = rule(
    implementation = _make_library_impl,
    doc = "Owns the Makefiles of one package for dx lint (checkmake) and dx format (mbake).",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Makefiles of this package: Makefile, makefile, GNUmakefile or *.mk.",
        ),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Other make libraries this package depends on."),
    },
)
