"""Make variables with the cc environment for cargo build scripts."""

load("@apple_support//lib:apple_support.bzl", "apple_support")
load("@bazel_skylib//lib:paths.bzl", "paths")
load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

BUILD_SCRIPT_CC_ENV = {
    "CFLAGS": "$(DX_BS_CFLAGS)",
    "HOST_AR": "$(DX_BS_HOST_AR)",
    "HOST_CC": "$(DX_BS_HOST_CC)",
    "HOST_CFLAGS": "$(DX_BS_HOST_CFLAGS)",
    "HOST_CXX": "$(DX_BS_HOST_CXX)",
    "HOST_CXXFLAGS": "$(DX_BS_HOST_CXXFLAGS)",
    "TARGET_AR": "$(DX_BS_TARGET_AR)",
    "TARGET_CC": "$(DX_BS_TARGET_CC)",
    "TARGET_CXX": "$(DX_BS_TARGET_CXX)",
}

_UNDEF_ISO_STDIO = "/U_CRT_STDIO_ISO_WIDE_SPECIFIERS"

_PATH_PLACEHOLDERS = [
    apple_support.path_placeholders.xcode(),
    apple_support.path_placeholders.sdkroot(),
]

def _should_prefix_pwd(path):
    if paths.is_absolute(path) or path.startswith("${pwd}"):
        return False
    for placeholder in _PATH_PLACEHOLDERS:
        if path.startswith(placeholder):
            return False
    return True

def _pwd_tool(path):
    return "${pwd}/" + path if _should_prefix_pwd(path) else path

def _prefix_pwd_to_flag(args, flags):
    res = []
    prefix_next = False
    for arg in args:
        if prefix_next and arg in ("-Xclang", "-Xpreprocessor"):
            res.append(arg)
            continue
        handled = False
        next_prefix = False
        for flag in sorted(flags, key = len, reverse = True):
            spaced = not (flag.endswith("=") or flag.endswith(":"))
            if arg == flag:
                next_prefix = spaced
                res.append(arg)
                handled = True
                break
            if arg.startswith(flag):
                path = arg[len(flag):].strip()
                res.append(flag + "${pwd}/" + path if _should_prefix_pwd(path) else arg)
                handled = True
                break
            if spaced and prefix_next and not arg.strip().startswith("-") and _should_prefix_pwd(arg.strip()):
                res.append("${pwd}/" + arg.strip())
                handled = True
                break
        if not handled:
            res.append(arg)
        prefix_next = next_prefix
    return res

_PWD_FLAGS = [
    ["--sysroot=", "--sysroot", "-isysroot"],
    ["-resource-dir=", "-resource-dir"],
    ["-B"],
    ["-LIBPATH=", "-LIBPATH:", "-LIBPATH", "/LIBPATH:", "/LIBPATH", "-L"],
    ["@"],
    ["-isystem", "-isystem-after", "-internal-isystem", "-internal-externc-isystem", "-cxx-isystem", "-stdlib++-isystem", "/imsvc"],
    ["-fsanitize-ignorelist="],
    ["-imacros"],
    ["-ivfsoverlay", "-vfsoverlay=", "-vfsoverlay", "/vfsoverlay:", "/vfsoverlay"],
]

_DIRECT_LIB_EXTENSIONS = (".a", ".o", ".so", ".dylib")

def _pwd_flags(args):
    prefix = "/clang:"
    wrapped = [arg.startswith(prefix) for arg in args]
    args = [arg[len(prefix):] if wrapped[i] else arg for i, arg in enumerate(args)]
    for flags in _PWD_FLAGS:
        args = _prefix_pwd_to_flag(args, flags)
    args = [
        "${pwd}/" + arg if not arg.startswith("-") and _should_prefix_pwd(arg) and arg.endswith(_DIRECT_LIB_EXTENSIONS) else arg
        for arg in args
    ]
    return " ".join([prefix + arg if wrapped[i] else arg for i, arg in enumerate(args)])

def _configure(ctx, cc_toolchain):
    return cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )

def _tools(cc_toolchain, feature_configuration):
    tools = {
        "AR": cc_common.get_tool_for_action(
            feature_configuration = feature_configuration,
            action_name = ACTION_NAMES.cpp_link_static_library,
        ) or cc_toolchain.ar_executable,
        "CC": cc_common.get_tool_for_action(
            feature_configuration = feature_configuration,
            action_name = ACTION_NAMES.c_compile,
        ),
        "CXX": cc_common.get_tool_for_action(
            feature_configuration = feature_configuration,
            action_name = ACTION_NAMES.cpp_compile,
        ),
    }
    return {k: v if "libtool" in v else _pwd_tool(v) for k, v in tools.items()}

def _compile_flags(cc_toolchain, feature_configuration, action_name):
    return _pwd_flags(cc_common.get_memory_inefficient_command_line(
        feature_configuration = feature_configuration,
        action_name = action_name,
        variables = cc_common.create_compile_variables(
            feature_configuration = feature_configuration,
            cc_toolchain = cc_toolchain,
        ),
    ))

def _build_script_cc_env_impl(ctx):
    rust = ctx.toolchains["@rules_rust//rust:toolchain_type"]
    cc_toolchain = find_cc_toolchain(ctx)
    target_tools = _tools(cc_toolchain, _configure(ctx, cc_toolchain))
    cross = rust.target_flag_value != rust.exec_triple.str
    coverage = ctx.configuration.coverage_enabled
    exec_cc_toolchain = ctx.attr._exec_cc_toolchain[cc_common.CcToolchainInfo]
    if cross:
        exec_features = _configure(ctx, exec_cc_toolchain)
        host_tools = _tools(exec_cc_toolchain, exec_features)
        host_cflags = _compile_flags(exec_cc_toolchain, exec_features, ACTION_NAMES.c_compile)
        host_cxxflags = _compile_flags(exec_cc_toolchain, exec_features, ACTION_NAMES.cpp_compile)
        if coverage and rust.exec_triple.str.endswith("-windows-msvc"):
            host_cflags += " " + _UNDEF_ISO_STDIO
    else:
        host_tools = target_tools
        host_cflags = ""
        host_cxxflags = ""
    variables = {
        "DX_BS_CFLAGS": _UNDEF_ISO_STDIO if coverage and rust.target_triple.str.endswith("-windows-msvc") else "",
        "DX_BS_HOST_CFLAGS": host_cflags,
        "DX_BS_HOST_CXXFLAGS": host_cxxflags,
    }
    for name in ["AR", "CC", "CXX"]:
        variables["DX_BS_HOST_" + name] = host_tools[name]
        variables["DX_BS_TARGET_" + name] = target_tools[name]
    return [
        platform_common.TemplateVariableInfo({k: v.replace("$", "$$") for k, v in variables.items()}),
        platform_common.ToolchainInfo(all_files = exec_cc_toolchain.all_files if cross else depset()),
    ]

build_script_cc_env = rule(
    implementation = _build_script_cc_env_impl,
    doc = "Publishes `DX_BS_*` make variables with the target and exec cc tools and flags for `cargo_build_script`.",
    attrs = {
        "_exec_cc_toolchain": attr.label(
            default = Label("@rules_cc//cc:current_cc_toolchain"),
            cfg = "exec",
        ),
    },
    fragments = ["cpp"],
    toolchains = use_cc_toolchain() + ["@rules_rust//rust:toolchain_type"],
)
