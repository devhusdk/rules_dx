"""clang-cl cc_toolchains for MSVC targets whose link actions carry the hermetic LIB search path."""

load("@bazel_features//:features.bzl", "bazel_features")
load("@llvm//platforms:common.bzl", "MSVC_TARGET_STAGE0_SUPPORTED_EXECS", "WINDOWS_TARGETS")
load("@llvm//toolchain:cc_toolchain.bzl", "cc_toolchain")
load("@llvm//toolchain:merged_resource_directory.bzl", "merged_resource_directory")
load("@llvm//toolchain:selects.bzl", "platform_cc_tool_map", "platform_module_map", "platform_resource_dir")
load("@llvm//toolchain/args:resource_directory_args.bzl", "resource_directory_args")
load("@rules_cc//cc/toolchains:tool.bzl", "cc_tool")
load("@rules_cc//cc/toolchains:tool_map.bzl", "cc_tool_map")

def _tool_map(prefix, exec_os, exec_cpu):
    suffix = ".exe" if exec_os == "windows" else ""
    resource_dir = platform_resource_dir(exec_os, exec_cpu)

    def binary(name):
        return resource_dir.same_package_label("bin/" + name + suffix)

    cc_tool(
        name = prefix + "_clang_cl",
        src = binary("clang-cl"),
        data = [
            resource_dir,
            binary("lld-link"),
        ],
        capabilities = [
            "@rules_cc//cc/toolchains/capabilities:has_configured_linker_path",
            "@rules_cc//cc/toolchains/capabilities:supports_dynamic_linker",
            "@rules_cc//cc/toolchains/capabilities:supports_interface_shared_libraries",
        ],
        allowlist_include_directories = [
            resource_dir,
            resource_dir.same_package_label("builtin_resource_include_dir"),
        ],
    )

    cc_tool(
        name = prefix + "_llvm_lib",
        src = binary("llvm-lib"),
        env = {"LIB": "__hermetic_llvm_empty_lib__"},
    )

    cc_tool(
        name = prefix + "_llvm_strip",
        src = binary("llvm-strip"),
        data = select({
            "@llvm//toolchain:runtimes_all": ["@llvm//tools/internal:static-library-validator"],
            "//conditions:default": [],
        }) + [binary(tool) for tool in ["c++filt", "llvm-nm", "gcov", "llvm-cov", "llvm-profdata"]],
    )

    cc_tool(
        name = prefix + "_def_file_generator",
        src = "@llvm//tools/def_file_generator",
        data = [binary("llvm-nm")],
        env = {"LLVM_NM": "{llvm_nm}"},
        format = {"llvm_nm": binary("llvm-nm")},
    )

    cc_tool(
        name = prefix + "_static_library_validator",
        src = "@llvm//tools/internal:static-library-validator",
        data = [binary("c++filt"), binary("llvm-nm")],
        env = {
            "LLVM_CXXFILT": "{cxxfilt}",
            "LLVM_NM": "{llvm_nm}",
        },
        format = {
            "cxxfilt": binary("c++filt"),
            "llvm_nm": binary("llvm-nm"),
        },
    )

    clang_cl = ":" + prefix + "_clang_cl"
    construction_tools = {
        "@rules_cc//cc/toolchains/actions:" + action: clang_cl
        for action in [
            "assemble",
            "c_compile",
            "cpp_compile",
            "linkstamp_compile",
            "lto_backend",
            "preprocess_assemble",
            "cpp_header_parsing",
            "cpp_link_executable",
            "cpp_link_dynamic_library",
            "cpp_link_nodeps_dynamic_library",
            "objc_executable",
            "lto_index_for_executable",
            "lto_index_for_dynamic_library",
            "lto_index_for_nodeps_dynamic_library",
        ]
    } | {
        "@rules_cc//cc/toolchains/actions:ar_actions": ":" + prefix + "_llvm_lib",
        "@rules_cc//cc/toolchains/actions:strip": ":" + prefix + "_llvm_strip",
    }
    validator = {
        "@rules_cc//cc/toolchains/actions:validate_static_library": ":" + prefix + "_static_library_validator",
    } if bazel_features.cc.supports_starlarkified_toolchains else {}

    cc_tool_map(
        name = prefix + "_complete_tools",
        tools = construction_tools | validator | {
            "@rules_cc//cc/toolchains/actions:generate_def_file": ":" + prefix + "_def_file_generator",
        },
    )

    cc_tool_map(
        name = prefix + "_construction_tools",
        tools = construction_tools,
    )

    native.alias(
        name = prefix + "_tools",
        actual = select({
            "@llvm//toolchain:runtimes_all": ":" + prefix + "_complete_tools",
            "//conditions:default": ":" + prefix + "_construction_tools",
        }),
    )
    return ":" + prefix + "_tools"

def clang_cl_toolchains(link_lib_env):
    """Declares one clang-cl cc_toolchain per LLVM execution platform for MSVC targets.

    Args:
        link_lib_env: cc_args setting LIB on link actions.
    """
    for (exec_os, exec_cpu) in MSVC_TARGET_STAGE0_SUPPORTED_EXECS:
        prefix = exec_os + "_" + exec_cpu
        case_overlays = [] if exec_os == "windows" else ["@llvm//toolchain/args/windows/msvc:case_overlays"]
        msvc_args = [
            "@llvm//toolchain/args/windows/msvc:normalized_default_libs_for_runtime",
            "@llvm//toolchain/args/windows/msvc:normalized_sdk_compile_args",
            link_lib_env,
        ] + case_overlays

        merged_resource_directory(
            name = prefix + "_resource_directory",
            parent = platform_resource_dir(exec_os, exec_cpu),
            srcs = ["@llvm//runtimes:resource_directory"],
        )
        resource_directory_args(
            name = prefix + "_resource_directory_args",
            compile_directory = platform_resource_dir(exec_os, exec_cpu),
            link_directory = ":" + prefix + "_resource_directory",
        )

        native.alias(
            name = prefix + "_llvm_tools",
            actual = platform_cc_tool_map(exec_os, exec_cpu),
        )

        cc_toolchain(
            name = prefix + "_cc_toolchain",
            tool_map = select({
                "@llvm//constraints/windows/abi:msvc": _tool_map(prefix, exec_os, exec_cpu),
                "//conditions:default": ":" + prefix + "_llvm_tools",
            }),
            module_map = platform_module_map(exec_os, exec_cpu),
            supports_gnu_param_file_quoting = exec_os in ["linux", "macos"],
            extra_args = [":" + prefix + "_resource_directory_args"] + select({
                "@llvm//platforms/config:windows_x86_64_msvc": msvc_args,
                "@llvm//platforms/config:windows_aarch64_msvc": msvc_args,
                "//conditions:default": [],
            }),
        )

        for (target_os, target_cpu) in WINDOWS_TARGETS:
            native.toolchain(
                name = prefix + "_to_" + target_os + "_" + target_cpu,
                exec_compatible_with = [
                    "@platforms//cpu:" + exec_cpu,
                    "@platforms//os:" + exec_os,
                ],
                target_compatible_with = [
                    "@platforms//cpu:" + target_cpu,
                    "@platforms//os:" + target_os,
                ],
                target_settings = [
                    "//config:toolchains_on",
                    "@llvm//toolchain:bootstrap_stage0_prebuilt_seed",
                    "@llvm//platforms/config:windows_" + target_cpu + "_msvc",
                ],
                toolchain = ":" + prefix + "_cc_toolchain",
                toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
            )
