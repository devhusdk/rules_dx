"""Analysis-time checks for dx CUDA targets."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cuda//cuda/private:providers.bzl", "CudaToolkitInfo")
load("//modules:cuda.bzl", "CUDA_ARCHS", "CUDA_VERSION")

CudaLicenseInfo = provider(
    doc = "Whether the CUDA Toolkit EULA is accepted.",
    fields = {"error": "Why the toolkit is unusable. Empty when accepted."},
)

def _license_impl(ctx):
    return [CudaLicenseInfo(error = ctx.attr.error)]

cuda_license = rule(
    implementation = _license_impl,
    doc = "Records the CUDA Toolkit EULA state.",
    attrs = {
        "error": attr.string(doc = "Why the toolkit is unusable. Empty when accepted."),
    },
)

_TOOLCHAIN_TYPE = "@rules_cuda//cuda:toolchain_type"

def arch_errors(archs):
    """Returns one error per arch spec token that the pinned nvcc does not support."""
    if not archs:
        return ["dx_cuda: archs is empty; set archs, for example archs = [\"sm_89\"]"]
    errors = []
    for spec in archs:
        for part in spec.replace(":", ",").split(","):
            token = part.strip()
            if token not in CUDA_ARCHS:
                errors.append("dx_cuda: arch '" + token + "' is not supported by nvcc " + CUDA_VERSION + "; supported: " + ", ".join(CUDA_ARCHS))
    return errors

def compiler_errors(compiler):
    """Returns an error when rules_cuda is set to a compiler other than nvcc."""
    if compiler != "nvcc":
        return ["dx_cuda: @rules_cuda//cuda:compiler is '" + compiler + "'; dx CUDA targets compile with nvcc only"]
    return []

def toolkit_errors(nvcc_path, toolkit_path, version):
    """Returns errors for a CUDA toolchain that is not the pinned nvcc toolkit."""
    if toolkit_path.startswith("/") or not nvcc_path.endswith("+cuda_nvcc/nvcc/bin/nvcc"):
        return ["dx_cuda: the CUDA toolkit is not the pinned redistributable (nvcc at '" + nvcc_path + "'); declare the cuda.component and cuda.toolkit tags from the dx CUDA README"]
    if version != CUDA_VERSION:
        return ["dx_cuda: the CUDA toolkit is " + version + "; dx CUDA targets need " + CUDA_VERSION]
    return []

def _check_impl(ctx):
    license = ctx.attr.license[CudaLicenseInfo]
    if license.error:
        fail(license.error)
    errors = compiler_errors(ctx.attr._compiler[BuildSettingInfo].value) + arch_errors(ctx.attr.archs)
    if errors:
        fail("\n".join(errors))
    return [DefaultInfo()]

cuda_check = rule(
    implementation = _check_impl,
    doc = "Fails analysis when a dx CUDA target uses an unsupported arch, compiler or license state.",
    attrs = {
        "archs": attr.string_list(doc = "Arch specs of the checked target."),
        "license": attr.label(
            doc = "CUDA Toolkit EULA state.",
            default = "@dx_cuda_license//:license",
            providers = [CudaLicenseInfo],
        ),
        "_compiler": attr.label(default = "@rules_cuda//cuda:compiler"),
    },
)

def _toolkit_check_impl(ctx):
    toolchain = ctx.toolchains[_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail("dx_cuda: no nvcc toolchain for this target; dx CUDA targets build for linux x86_64")
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
