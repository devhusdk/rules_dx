"""Windows CUDA runtime that links cudart_static.lib and cudadevrt.lib."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

_LIBRARIES = ["cudart_static.lib", "cudadevrt.lib"]

def _static_runtime_impl(ctx):
    files = ctx.files.files
    libraries = []
    for name in _LIBRARIES:
        found = [f for f in files if f.basename.lower() == name]
        if len(found) != 1:
            candidates = sorted([f.path for f in files if f.basename.lower().endswith(".lib")])
            fail("cuda_windows_static_runtime: expected exactly one {} in {}, found {}; .lib candidates: {}".format(
                name,
                ctx.attr.files.label,
                len(found),
                ", ".join(candidates) or "none",
            ))
        libraries.append(cc_common.create_library_to_link(
            actions = ctx.actions,
            static_library = found[0],
        ))
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        libraries = depset(libraries),
    )
    return [CcInfo(
        compilation_context = ctx.attr.runtime[CcInfo].compilation_context,
        linking_context = cc_common.create_linking_context(linker_inputs = depset([linker_input])),
    )]

cuda_windows_static_runtime = rule(
    implementation = _static_runtime_impl,
    doc = "CUDA runtime for Windows that links cudart_static.lib and cudadevrt.lib with the headers of `runtime`.",
    attrs = {
        "files": attr.label(
            doc = "Files of the cudart component holding cudart_static.lib and cudadevrt.lib.",
            mandatory = True,
            allow_files = True,
        ),
        "runtime": attr.label(
            doc = "CUDA runtime whose compilation context is reused.",
            mandatory = True,
            providers = [CcInfo],
        ),
    },
    provides = [CcInfo],
)
