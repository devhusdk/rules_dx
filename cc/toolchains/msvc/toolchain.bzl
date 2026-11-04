"""cl.exe C++ toolchain over a fixed MSVC and Windows SDK layout."""

load("@rules_cc//cc/toolchains:cc_toolchain.bzl", "cc_toolchain")
load("@rules_cc//cc/private/toolchain:windows_cc_toolchain_config.bzl", "cc_toolchain_config")
load(":layout.bzl", "msvc_layout")

# Qt 6 qcompilerdetection.h requires a conforming __cplusplus and /permissive-.
MSVC_CXX_CONFORMANCE_FLAGS = ["/Zc:__cplusplus", "/permissive-"]

def msvc_cc_toolchain(name, msvc_version, sdk_version, tmp = "", target_cpu = "x64", **kwargs):
    """Declare a cc_toolchain that runs cl.exe from the layout in this package.

    The layout root is the package directory: VC and Windows Kits sit next to
    the BUILD file. Only the x64 target is supported. C++ actions, not C
    actions, get MSVC_CXX_CONFORMANCE_FLAGS.

    Args:
        name: Name of the cc_toolchain.
        msvc_version: MSVC toolset version directory under VC/Tools/MSVC.
        sdk_version: Windows SDK version directory.
        tmp: Value for TMP and TEMP in compile and link actions.
        target_cpu: Target CPU; only "x64" is supported.
        **kwargs: Passed to cc_toolchain, such as visibility.
    """
    if target_cpu != "x64":
        fail("msvc_cc_toolchain supports target_cpu = \"x64\" only, got %r" % target_cpu)
    layout = msvc_layout(msvc_version, sdk_version)
    root = native.repository_name().removeprefix("@")
    root = ("external/" + root + "/" if root else "") + native.package_name()
    root = root + "/" if native.package_name() else root

    def exec_paths(paths):
        return ";".join([(root + p).replace("/", "\\") for p in paths])

    cc_toolchain_config(
        name = name + "_config",
        cpu = "x64_windows",
        compiler = "msvc-cl",
        host_system_name = "local",
        target_system_name = "local",
        target_libc = "ucrt",
        abi_version = "local",
        abi_libc_version = "local",
        toolchain_identifier = name,
        msvc_env_tmp = tmp,
        msvc_env_path = exec_paths(layout.bin_dirs),
        msvc_env_include = exec_paths(layout.include_dirs),
        msvc_env_lib = exec_paths(layout.lib_dirs),
        msvc_cl_path = layout.cl,
        msvc_ml_path = layout.ml64,
        msvc_link_path = layout.link,
        msvc_lib_path = layout.lib,
        cxx_builtin_include_directories = layout.include_dirs,
        tool_paths = {
            "ar": layout.lib,
            "ml": layout.ml64,
            "cpp": layout.cl,
            "gcc": layout.cl,
            "ld": layout.link,
        },
        archiver_flags = ["/MACHINE:X64"],
        default_link_flags = ["/MACHINE:X64"],
        cxx_flags = MSVC_CXX_CONFORMANCE_FLAGS,
        dbg_mode_debug_flag = "/DEBUG",
        fastbuild_mode_debug_flag = "/DEBUG",
        supports_parse_showincludes = True,
        shorten_virtual_includes = True,
    )
    native.filegroup(
        name = name + "_files",
        srcs = native.glob(["VC/**", "Windows Kits/**"], allow_empty = True),
    )
    cc_toolchain(
        name = name,
        toolchain_config = ":" + name + "_config",
        toolchain_identifier = name,
        all_files = ":" + name + "_files",
        ar_files = ":" + name + "_files",
        as_files = ":" + name + "_files",
        compiler_files = ":" + name + "_files",
        dwp_files = ":" + name + "_files",
        linker_files = ":" + name + "_files",
        objcopy_files = ":" + name + "_files",
        strip_files = ":" + name + "_files",
        supports_param_files = 1,
        **kwargs
    )
