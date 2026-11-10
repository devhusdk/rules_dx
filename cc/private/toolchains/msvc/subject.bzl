"""Analysis subject reporting what an MSVC cc_toolchain would run."""

load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("//cc/private/toolchains:bindings.bzl", "cc_compiler_identity")
load("//core:defs.bzl", "DxSubjectInfo")

_TOOL_ACTIONS = {
    "tool.c_compile": ACTION_NAMES.c_compile,
    "tool.cpp_compile": ACTION_NAMES.cpp_compile,
    "tool.archive": ACTION_NAMES.cpp_link_static_library,
    "tool.exe_link": ACTION_NAMES.cpp_link_executable,
    "tool.dll_link": ACTION_NAMES.cpp_link_dynamic_library,
}

def _features(ctx, cc_toolchain, requested):
    return cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = requested,
    )

def _compile_flags(ctx, cc_toolchain, requested, action_name = ACTION_NAMES.cpp_compile, keep = ["/MD", "/MDd", "/MT", "/MTd"]):
    config = _features(ctx, cc_toolchain, requested)
    variables = cc_common.create_compile_variables(
        feature_configuration = config,
        cc_toolchain = cc_toolchain,
        source_file = "a.c" if action_name == ACTION_NAMES.c_compile else "a.cc",
        output_file = "a.obj",
    )
    flags = cc_common.get_memory_inefficient_command_line(
        feature_configuration = config,
        action_name = action_name,
        variables = variables,
    )
    return [f for f in flags if f in keep]

def _env(config, action, variables):
    return cc_common.get_environment_variables(
        feature_configuration = config,
        action_name = action,
        variables = variables,
    )

def _msvc_toolchain_subject_impl(ctx):
    cc_toolchain = ctx.attr.toolchain[cc_common.CcToolchainInfo]
    config = _features(ctx, cc_toolchain, [])
    fields = {
        key: cc_common.get_tool_for_action(feature_configuration = config, action_name = action)
        for key, action in _TOOL_ACTIONS.items()
    }
    compile_variables = cc_common.create_compile_variables(
        feature_configuration = config,
        cc_toolchain = cc_toolchain,
        source_file = "a.cc",
        output_file = "a.obj",
    )
    link_variables = cc_common.create_link_variables(
        cc_toolchain = cc_toolchain,
        feature_configuration = config,
        output_file = "a.dll",
        is_linking_dynamic_library = True,
    )
    compile_env = _env(config, ACTION_NAMES.cpp_compile, compile_variables)
    link_env = _env(config, ACTION_NAMES.cpp_link_dynamic_library, link_variables)
    fields["env.INCLUDE"] = compile_env.get("INCLUDE", "")
    fields["env.LIB"] = link_env.get("LIB", "")
    fields["env.PATH"] = compile_env.get("PATH", "")
    fields["env.TMP"] = compile_env.get("TMP", "")
    fields["flags.default"] = " ".join(_compile_flags(ctx, cc_toolchain, []))
    fields["flags.dbg"] = " ".join(_compile_flags(ctx, cc_toolchain, ["dbg"]))
    fields["flags.static_link_msvcrt"] = " ".join(_compile_flags(ctx, cc_toolchain, ["static_link_msvcrt"]))
    conformance = ["/Zc:__cplusplus", "/permissive-"]
    fields["flags.cpp_conformance"] = " ".join(_compile_flags(ctx, cc_toolchain, [], ACTION_NAMES.cpp_compile, conformance)) or "none"
    fields["flags.c_conformance"] = " ".join(_compile_flags(ctx, cc_toolchain, [], ACTION_NAMES.c_compile, conformance)) or "none"
    dll_line = cc_common.get_memory_inefficient_command_line(
        feature_configuration = config,
        action_name = ACTION_NAMES.cpp_link_dynamic_library,
        variables = link_variables,
    )
    fields["dll_link.has_dll_flag"] = str("/DLL" in dll_line)
    fields["cc.compiler"] = cc_toolchain.compiler
    fields["cc.cpu"] = cc_toolchain.cpu
    fields["cc.target_system"] = cc_toolchain.target_gnu_system_name
    fields["flags.machine"] = " ".join([f for f in dll_line if f.startswith("/MACHINE:")])
    fields["cc.family"] = cc_compiler_identity(cc_toolchain.compiler, "").family
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = fields),
    ]

msvc_toolchain_subject = rule(
    implementation = _msvc_toolchain_subject_impl,
    doc = "Analysis subject reporting tools, environment and flags of an MSVC cc_toolchain.",
    attrs = {"toolchain": attr.label(mandatory = True, providers = [cc_common.CcToolchainInfo])},
    fragments = ["cpp"],
)
