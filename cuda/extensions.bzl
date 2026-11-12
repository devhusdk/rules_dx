"""Public CUDA module extensions."""

load("@rules_cuda//cuda:platform_alias_extension.bzl", "platform_alias_repo")
load("@rules_cuda//cuda/private:platforms.bzl", "SUPPORTED_PLATFORMS")
load("@rules_cuda//cuda/private:repositories.bzl", "cuda_component", "cuda_toolkit")
load("//cuda/private:license.bzl", "cuda_unaccepted")
load("//cuda/private/toolchain:platforms.bzl", "CUDA_PLATFORMS")

_LICENSE = "cuda-eula"

_license_tag = tag_class(attrs = {
    "accept": attr.string_list(mandatory = True, doc = "License names to accept: cuda-eula."),
})

def cuda_license_error(modules):
    """Returns the error for an unaccepted CUDA Toolkit EULA, or an empty string when the root module accepts it."""
    names = []
    for module in modules:
        if module.is_root:
            for tag in module.tags.license:
                names.extend(tag.accept)
    for name in names:
        if name != _LICENSE:
            fail("dx_cuda: unknown license '" + name + "'; the only one is '" + _LICENSE + "'")
    if _LICENSE in names:
        return ""
    return "dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda.license(accept = [\"cuda-eula\"]) to the root MODULE.bazel"

def cuda_component_repo_name(component, platform, version):
    """Returns the repo name of a pinned component, the same for the real repo and the license stub."""
    return "cuda_{}_{}_{}".format(component, platform.replace("-", "_"), version.replace(".", "_"))

def cuda_component_repo(name, license_error, **kwargs):
    """Declares a component repo, or the stub that fails its fetch when the license is unaccepted."""
    if license_error:
        cuda_unaccepted(name = name, error = license_error)
    else:
        cuda_component(name = name, **kwargs)

def _impl(module_ctx):
    pins = json.decode(module_ctx.read(Label("//cuda/private/toolchain:pins.json")))
    version = pins["version"]
    version_label = version.replace(".", "_")
    license_error = cuda_license_error(module_ctx.modules)

    repos = {}
    for cell, cell_pins in pins["cells"].items():
        platform = CUDA_PLATFORMS[cell]
        for component, pin in cell_pins["components"].items():
            repo = cuda_component_repo_name(component, platform, version)
            cuda_component_repo(
                name = repo,
                license_error = license_error,
                component_name = component,
                urls = pin["urls"],
                sha256 = pin["sha256"],
                strip_prefix = pin["strip_prefix"],
                version = pin["version"],
                toolkit_version = version,
            )
            repos.setdefault(component, {})[platform] = repo

    exact = {}
    facade = {}
    for component, by_platform in repos.items():
        for name, alias in (("cuda_{}_toolchain_{}".format(component, version_label), exact), ("cuda_" + component, facade)):
            platform_alias_repo(
                name = name,
                component_name = component,
                versions = [version],
                **{p.replace("-", "_") + "_repos": ({version: by_platform[p]} if p in by_platform else {}) for p in SUPPORTED_PLATFORMS}
            )
            alias[component] = "@" + name

    impl_name = "cuda_toolchain_" + version_label
    cuda_toolkit(
        name = impl_name,
        components_mapping = exact,
        version = version,
    )
    cuda_toolkit(
        name = "cuda",
        components_mapping = facade,
        version = version,
        toolchain_repositories = {version: "@" + impl_name},
    )

cuda = module_extension(
    implementation = _impl,
    tag_classes = {"license": _license_tag},
    doc = "Declares the pinned CUDA Toolkit components and the toolkit repo `cuda` from cuda/private/toolchain/pins.json.",
)
