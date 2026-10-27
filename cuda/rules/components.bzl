"""License-gated CUDA library components (cuBLAS, cuFFT, cuRAND)."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//cuda/toolchain:check.bzl", "CudaLicenseInfo", "component_errors")
load("//libs/starlark:wrapper.bzl", "dx_preserved_providers")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _library_paths(cc_info):
    paths = [f.path for f in cc_info.compilation_context.headers.to_list()]
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        for library in linker_input.libraries:
            # The solib symlink path mangles the repo name; check the file it points at.
            shared = library.resolved_symlink_dynamic_library or library.dynamic_library
            for f in (shared, library.static_library, library.pic_static_library):
                if f != None:
                    paths.append(f.path)
    return paths

def _impl(ctx):
    license = ctx.attr.license[CudaLicenseInfo]
    if license.error:
        fail(license.error)
    upstream = ctx.attr.component
    providers = dx_preserved_providers(upstream, [(CcInfo, "CcInfo"), (DefaultInfo, "DefaultInfo")], "cuda_component_library")
    errors = component_errors(ctx.attr.component_name, _library_paths(providers[0]))
    if errors:
        fail("\n".join(errors))

    # Pinned external redistributable; dx owns none of its sources.
    return providers + [QualitySourcesInfo(direct_sources = {})]

cuda_component_library = rule(
    implementation = _impl,
    doc = "Forwards one pinned CUDA library component after the EULA gate and the pin check.",
    attrs = {
        "component": attr.label(
            doc = "The rules_cuda component target, for example @cuda//:cublas.",
            mandatory = True,
            providers = [CcInfo],
        ),
        "component_name": attr.string(doc = "rules_cuda short name of the component.", mandatory = True),
        "license": attr.label(
            doc = "CUDA Toolkit EULA state.",
            default = "@dx_cuda_license//:license",
            providers = [CudaLicenseInfo],
        ),
    },
    provides = [CcInfo, QualitySourcesInfo],
)
