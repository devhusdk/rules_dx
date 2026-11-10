"""Builds targets for linux arm64 from any host."""

load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

def _arm64_impl(_settings, _attr):
    return {"//command_line_option:platforms": str(Label("//appimage:linux_arm64"))}

_arm64 = transition(
    implementation = _arm64_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _files_impl(ctx):
    files = []
    for target in ctx.attr.srcs:
        files.extend(target[OutputGroupInfo].CMake_logs.to_list())
    return [DefaultInfo(files = depset(files))]

arm64_files = rule(
    implementation = _files_impl,
    doc = "Forwards the CMake logs of targets built for linux arm64.",
    attrs = {
        "srcs": attr.label_list(doc = "Targets built for linux arm64.", cfg = _arm64),
    },
)

def _wrong_arch_link_impl(ctx):
    archives = [f for f in ctx.attr.lib[0][DefaultInfo].files.to_list() if f.basename == ctx.attr.archive]
    if len(archives) != 1:
        fail("expected one %s in %s" % (ctx.attr.archive, ctx.attr.lib[0].label))
    cc_toolchain = find_cc_toolchain(ctx)
    config = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    log = ctx.actions.declare_file(ctx.label.name + ".log")
    variables = cc_common.create_link_variables(
        cc_toolchain = cc_toolchain,
        feature_configuration = config,
        output_file = log.path + ".bin",
        is_linking_dynamic_library = False,
        user_link_flags = ["-Wl,--whole-archive", archives[0].path, "-Wl,--no-whole-archive"],
    )
    tool = cc_common.get_tool_for_action(
        feature_configuration = config,
        action_name = ACTION_NAMES.cpp_link_executable,
    )
    args = cc_common.get_memory_inefficient_command_line(
        feature_configuration = config,
        action_name = ACTION_NAMES.cpp_link_executable,
        variables = variables,
    )
    env = cc_common.get_environment_variables(
        feature_configuration = config,
        action_name = ACTION_NAMES.cpp_link_executable,
        variables = variables,
    )
    command = " ".join([tool] + [_quote(a) for a in args])
    # run_shell: A redirect captures the linker output and exit code.
    ctx.actions.run_shell(
        outputs = [log],
        inputs = depset([archives[0]], transitive = [cc_toolchain.all_files]),
        env = env,
        command = "%s > %s 2>&1; echo exit=$? >> %s" % (command, _quote(log.path), _quote(log.path)),
        mnemonic = "WrongArchLink",
        progress_message = "Linking %s for the wrong architecture" % archives[0].short_path,
    )
    return [DefaultInfo(files = depset([log]))]

def _quote(value):
    return "'" + value.replace("'", "'\\''") + "'"

wrong_arch_link = rule(
    implementation = _wrong_arch_link_impl,
    doc = "Links an arm64 static archive into an executable with the target cc toolchain and saves the linker output and exit code to a log.",
    attrs = {
        "archive": attr.string(default = "libz.a", doc = "Basename of the archive to link."),
        "lib": attr.label(doc = "Target built for linux arm64.", cfg = _arm64),
    },
    fragments = ["cpp"],
    toolchains = use_cc_toolchain(),
)
