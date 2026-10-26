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

def _version(text):
    parts = text.split(".") if type(text) == "string" else []
    if len(parts) != 2 or not parts[0].isdigit() or not parts[1].isdigit():
        return None
    return (int(parts[0]), int(parts[1]))

def _lowest_arch(archs):
    lowest = None
    for spec in archs or []:
        for part in spec.replace(":", ",").split(","):
            token = part.strip()
            for prefix in ("sm_", "compute_"):
                number = token[len(prefix):]
                if token.startswith(prefix) and number.isdigit():
                    cc = (int(number) // 10, int(number) % 10)
                    if lowest == None or cc < lowest:
                        lowest = cc
    return lowest

def gpu_requirement_errors(archs, min_compute_capability, min_driver):
    """Returns errors for GPU test requirements that the archs or the pinned toolkit cannot meet."""
    errors = []
    cc = _version(min_compute_capability)
    lowest = _lowest_arch(archs)
    if cc == None:
        errors.append("dx_cuda: min_compute_capability '" + str(min_compute_capability) + "' is not X.Y, for example \"8.9\"")
    elif lowest != None and cc < lowest:
        errors.append("dx_cuda: min_compute_capability " + min_compute_capability + " is below the lowest arch in archs (%d.%d)" % lowest)
    driver = _version(min_driver)
    pinned = _version(CUDA_VERSION)
    floor = "%d.0" % pinned[0]
    if driver == None:
        errors.append("dx_cuda: min_driver '" + str(min_driver) + "' is not X.Y, for example \"" + floor + "\"")
    elif driver < (pinned[0], 0) or driver > pinned:
        errors.append("dx_cuda: min_driver " + min_driver + " is outside " + floor + " to " + CUDA_VERSION + " for the pinned toolkit")
    return errors

def _gpu_check_impl(ctx):
    errors = gpu_requirement_errors(ctx.attr.archs, ctx.attr.min_compute_capability, ctx.attr.min_driver)
    if errors:
        fail("\n".join(errors))
    return [DefaultInfo()]

cuda_gpu_check = rule(
    implementation = _gpu_check_impl,
    doc = "Fails analysis when a dx CUDA GPU test declares requirements its archs or the pinned toolkit cannot meet.",
    attrs = {
        "archs": attr.string_list(doc = "Arch specs of the checked test."),
        "min_compute_capability": attr.string(doc = "Lowest device compute capability, as X.Y."),
        "min_driver": attr.string(doc = "Lowest CUDA driver version, as X.Y."),
    },
)

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

def component_errors(name, paths):
    """Returns errors for a library component whose files are not from the pinned redistributable."""
    marker = "+cuda_" + name + "/"
    if not paths:
        return ["dx_cuda: component '" + name + "' has no files; declare the cuda.component tags from the dx CUDA README"]
    for path in paths:
        if marker not in path:
            return ["dx_cuda: component '" + name + "' is not the pinned redistributable (file '" + path + "'); declare the cuda.component and cuda.toolkit tags from the dx CUDA README"]
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
