"""Analysis-time check that the resolved CUDA toolchain is the pinned nvcc toolkit."""

load("@rules_cuda//cuda/private:providers.bzl", "CudaToolkitInfo")
load("//core/modules:cuda.bzl", "cuda_qualified_cells")
load(":check.bzl", "toolkit_errors")

_TOOLCHAIN_TYPE = "//cuda:toolchain_type"

def _toolkit_check_impl(ctx):
    toolchain = ctx.toolchains[_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail("dx_cuda: no nvcc toolchain for this target; dx CUDA targets build for " + ", ".join(cuda_qualified_cells()))
    toolkit = toolchain.cuda_toolkit[CudaToolkitInfo]
    errors = toolkit_errors(
        toolchain.compiler_executable,
        toolkit.path,
        "{}.{}".format(toolkit.version_major, toolkit.version_minor),
    )
    if errors:
        fail("\n".join(errors))
    return [DefaultInfo()]

cuda_toolkit_check = rule(
    implementation = _toolkit_check_impl,
    doc = "Fails analysis when the resolved CUDA toolchain is not the pinned nvcc toolkit.",
    toolchains = [config_common.toolchain_type(_TOOLCHAIN_TYPE, mandatory = False)],
)
