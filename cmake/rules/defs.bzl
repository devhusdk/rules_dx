"""CMake source ownership for dx quality."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources", "named_source_class_error")

def _cmake_library_impl(ctx):
    for f in ctx.files.srcs:
        error = named_source_class_error("cmake", f.basename)
        if error != "":
            fail("cmake_library " + str(ctx.label) + ": " + error)
    direct_sources = {}
    if len(ctx.files.srcs) > 0:
        direct_sources["cmake"] = depset(ctx.files.srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

cmake_library = rule(
    implementation = _cmake_library_impl,
    doc = "Owns the CMake files of one package for dx format (gersemi).",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "CMake files of this package: CMakeLists.txt or *.cmake.",
        ),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Other cmake libraries this package depends on."),
    },
)
