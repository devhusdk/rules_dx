"""cl.exe C++ toolchain over a fixed MSVC and Windows SDK layout."""

load("@rules_cc//cc/private/toolchain:windows_cc_toolchain_config.bzl", "cc_toolchain_config")
load("@rules_cc//cc/toolchains:cc_toolchain.bzl", "cc_toolchain")
load(":layout.bzl", "SUPPORTED_CELLS", "msvc_layout")

# Qt 6 qcompilerdetection.h requires a conforming __cplusplus and /permissive-.
MSVC_CXX_CONFORMANCE_FLAGS = ["/Zc:__cplusplus", "/permissive-"]

_SYSTEM_NAMES = {"x64": "x86_64-pc-windows-msvc", "arm64": "aarch64-pc-windows-msvc"}

def msvc_cc_toolchain(name, msvc_version, sdk_version, vc_repo = "", sdk_repo = "", files = None, tmp = "C:\\Temp", exec_cpu = "x64", target_cpu = "x64", **kwargs):
    """Declare a cc_toolchain that runs cl.exe from the msvc_runtime and windows_sdk layouts."""
    if (exec_cpu, target_cpu) not in SUPPORTED_CELLS:
        fail("msvc_cc_toolchain does not support exec_cpu = %r, target_cpu = %r" % (exec_cpu, target_cpu))
    package = native.repository_name().removeprefix("@")
    package = ("external/" + package + "/" if package else "") + native.package_name()
    package = package + "/" if native.package_name() else package

    def roots(prefix):
        return {
            "vc_root": prefix + vc_repo + "/" if vc_repo else "",
            "sdk_root": prefix + sdk_repo + "/" if sdk_repo else "",
        }

    layout = msvc_layout(msvc_version, sdk_version, exec_cpu, target_cpu, **roots("../" * package.count("/") + "external/"))
    exec_layout = msvc_layout(msvc_version, sdk_version, exec_cpu, target_cpu, **roots("external/"))
    system_name = _SYSTEM_NAMES[target_cpu]
    identifier = name + "-" + msvc_version
    machine = "/MACHINE:" + target_cpu.upper()

    def exec_paths(paths):
        return ";".join([(p if p.startswith("external/") else package + p).replace("/", "\\") for p in paths])

    cc_toolchain_config(
        name = name + "_config",
        cpu = target_cpu + "_windows",
        compiler = "msvc-cl",
        host_system_name = _SYSTEM_NAMES[exec_cpu],
        target_system_name = system_name,
        target_libc = "ucrt",
        abi_version = "local",
        abi_libc_version = "local",
        toolchain_identifier = identifier,
        msvc_env_tmp = tmp,
        msvc_env_path = exec_paths(exec_layout.bin_dirs),
        msvc_env_include = exec_paths(exec_layout.include_dirs),
        msvc_env_lib = exec_paths(exec_layout.lib_dirs),
        msvc_cl_path = layout.cl,
        msvc_ml_path = layout.ml or "msvc_arm64_toolchain_does_not_support_ml",
        msvc_link_path = layout.link,
        msvc_lib_path = layout.lib,
        cxx_builtin_include_directories = layout.include_dirs,
        archiver_flags = [machine],
        default_link_flags = [machine, "/PDBALTPATH:%%_PDB%%"],
        cxx_flags = MSVC_CXX_CONFORMANCE_FLAGS,
        dbg_mode_debug_flag = "/DEBUG",
        fastbuild_mode_debug_flag = "/DEBUG",
        supports_parse_showincludes = True,
        shorten_virtual_includes = True,
    )
    if files == None:
        tool_dirs = {d.split("/bin/Host")[0] + "/bin" if "/bin/Host" in d else d: True for d in layout.bin_dirs}.keys()
        files = native.glob([p + "/**" for p in layout.include_dirs + layout.lib_dirs + list(tool_dirs)] + ["sysroot/Contents/VC/Auxiliary/Build/*.bat"], allow_empty = True)
    native.filegroup(
        name = name + "_files",
        srcs = files,
    )
    cc_toolchain(
        name = name,
        toolchain_config = ":" + name + "_config",
        toolchain_identifier = identifier,
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
