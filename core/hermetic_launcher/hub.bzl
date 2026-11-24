"""The dx_launcher_toolchains hub: the Windows aarch64 hermetic_launcher stubs (hermeticbuild/hermetic-launcher#94)."""

load("//core:repo.bzl", "hub_repo")
load("//core:toolchain.bzl", "toolchain_target")

visibility(["//core/..."])

HUB_NAME = "dx_launcher_toolchains"

_CONSTRAINTS = [
    "@platforms//cpu:aarch64",
    "@platforms//os:windows",
]

def _label(label):
    return repr(str(Label(label)))

def launcher_hub():
    """Returns the BUILD file of the dx_launcher_toolchains hub."""
    target_settings = [str(Label("//config:toolchains_on")), str(Label("@hermetic_launcher//launcher/private:stub_source_prebuilt"))]
    return "\n".join([
        "load({}, \"stub_finalizer_toolchain\")".format(_label("@hermetic_launcher//launcher/private:stub_finalizer_toolchain.bzl")),
        "load({}, \"stub_template_toolchain\")".format(_label("@hermetic_launcher//launcher/private:stub_template_toolchain.bzl")),
        "",
        "stub_template_toolchain(",
        "    name = \"template_aarch64_windows\",",
        "    template_exe = {},".format(_label("@runfiles_stub_aarch64_windows//file")),
        ")",
        "",
        toolchain_target(
            name = "template_aarch64_windows_toolchain",
            target_compatible_with = _CONSTRAINTS,
            target_settings = target_settings,
            toolchain = ":template_aarch64_windows",
            toolchain_type = str(Label("@hermetic_launcher//launcher:template_toolchain_type")),
        ),
        toolchain_target(
            name = "template_aarch64_windows_exec_toolchain",
            exec_compatible_with = _CONSTRAINTS,
            target_settings = target_settings,
            toolchain = ":template_aarch64_windows",
            toolchain_type = str(Label("@hermetic_launcher//launcher:template_exec_toolchain_type")),
        ),
        "stub_finalizer_toolchain(",
        "    name = \"finalizer_aarch64_windows\",",
        "    finalizer = {},".format(_label("@finalize_stub_aarch64_windows//file")),
        ")",
        "",
        toolchain_target(
            name = "finalizer_aarch64_windows_toolchain",
            exec_compatible_with = _CONSTRAINTS,
            target_settings = target_settings,
            toolchain = ":finalizer_aarch64_windows",
            toolchain_type = str(Label("@hermetic_launcher//launcher:finalizer_toolchain_type")),
        ),
    ])

def _dx_launchers_impl(module_ctx):
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": launcher_hub()})
    return module_ctx.extension_metadata(reproducible = True)

dx_launchers = module_extension(
    implementation = _dx_launchers_impl,
    doc = "Declares the `dx_launcher_toolchains` hub that rules_dx registers for consumers.",
)
