"""The `dx_protoc_toolchains` hub: protobuf's prebuilt protoc for every exec platform, gated by toolchains_on."""

load("//core:repo.bzl", "hub_repo")

_PLATFORMS = [
    ("linux_aarch64", "linux", "aarch64", "linux_aarch_64"),
    ("linux_ppc64le", "linux", "ppc64le", "linux_ppcle_64"),
    ("linux_s390x", "linux", "s390x", "linux_s390_64"),
    ("linux_x86_32", "linux", "x86_32", "linux_x86_32"),
    ("linux_x86_64", "linux", "x86_64", "linux_x86_64"),
    ("macos_aarch64", "macos", "aarch64", "osx_aarch_64"),
    ("macos_x86_64", "macos", "x86_64", "osx_x86_64"),
    ("windows_x86_32", "windows", "x86_32", "win32"),
    ("windows_x86_64", "windows", "x86_64", "win64"),
    ("windows_arm64", "windows", "arm64", "win64"),
]

_TOOLCHAIN = """
toolchain(
    name = "{name}",
    exec_compatible_with = ["@platforms//os:{os}", "@platforms//cpu:{cpu}"],
    target_settings = ["{toolchains_on}"],
    toolchain = "{toolchain}",
    toolchain_type = "{toolchain_type}",
)"""

def _build():
    lines = ['package(default_visibility = ["//visibility:public"])']
    for name, os, cpu, repo in _PLATFORMS:
        lines.append(_TOOLCHAIN.format(
            name = name,
            os = os,
            cpu = cpu,
            toolchains_on = str(Label("//config:toolchains_on")),
            toolchain = str(Label("@prebuilt_protoc.{}//:prebuilt_protoc_toolchain".format(repo))),
            toolchain_type = str(Label("@protobuf//bazel/private:proto_toolchain_type")),
        ))
    return "\n".join(lines) + "\n"

def _impl(mctx):
    hub_repo(name = "dx_protoc_toolchains", files = {"BUILD.bazel": _build()})
    return mctx.extension_metadata(reproducible = True)

protoc = module_extension(
    implementation = _impl,
    doc = "Declares the `dx_protoc_toolchains` hub that rules_dx registers for consumers.",
)
