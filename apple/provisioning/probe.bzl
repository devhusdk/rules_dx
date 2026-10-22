"""Probes the resolved Xcode, SDK, compiler, and executor behind one Apple provisioning."""

load("@apple_support//lib:apple_support.bzl", "apple_support")
load("@apple_support//xcode:providers.bzl", "XcodeVersionInfo")
load("@rules_cc//cc:action_names.bzl", "C_COMPILE_ACTION_NAME")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load(":defs.bzl", "AppleToolingInfo", "MANAGED_ROUTE", "deployment_floor", "version_below", "version_matches")

PLATFORMS = {
    "iphoneos": str(Label("@apple_support//platforms:ios_arm64")),
    "iphonesimulator": str(Label("@apple_support//platforms:ios_sim_arm64")),
    "macosx": str(Label("@apple_support//platforms:macos_arm64")),
}

MINIMUM_OS_FLAGS = {
    "iphoneos": "--ios_minimum_os",
    "iphonesimulator": "--ios_minimum_os",
    "macosx": "--macos_minimum_os",
}

_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"

def _has(ctx, constraint):
    return ctx.target_platform_has_constraint(constraint[platform_common.ConstraintValueInfo])

def _target_sdk(ctx):
    if _has(ctx, ctx.attr._macos):
        return "macosx"
    if _has(ctx, ctx.attr._ios):
        return "iphonesimulator" if _has(ctx, ctx.attr._simulator) else "iphoneos"
    return ""

def _platform(sdk):
    if sdk == "macosx":
        return apple_common.platform.macos, apple_common.platform_type.macos
    if sdk == "iphonesimulator":
        return apple_common.platform.ios_simulator, apple_common.platform_type.ios
    return apple_common.platform.ios_device, apple_common.platform_type.ios

def _missing_xcode(tooling):
    if tooling.route == "managed":
        return "apple: no Xcode resolved for this build; the managed route needs --xcode_version_config naming the Xcode on " + tooling.endpoint
    want = " " + tooling.xcode_version if tooling.xcode_version else ""
    return "apple: no Xcode resolved for this build; the local route needs a " + tooling.host + " host with Xcode" + want + " installed"

def _apple_toolchain_probe_impl(ctx):
    tooling = ctx.attr.provisioning[AppleToolingInfo]
    xcode_config = ctx.attr._xcode_config[XcodeVersionInfo]
    if xcode_config.xcode_version() == None:
        fail(_missing_xcode(tooling))
    xcode = str(xcode_config.xcode_version())
    if tooling.xcode_version and not version_matches(tooling.xcode_version, xcode):
        fail("apple: provisioning selects Xcode {} but Bazel resolved {}; pass --xcode_version={} or update xcode_version".format(
            tooling.xcode_version,
            xcode,
            tooling.xcode_version,
        ))
    sdk = _target_sdk(ctx)
    if sdk != tooling.sdk:
        fail("apple: provisioning selects {} but this build targets {}; build with --platforms={}".format(
            tooling.sdk,
            sdk or "no Apple platform",
            PLATFORMS[tooling.sdk],
        ))
    platform, platform_type = _platform(sdk)
    sdk_version = str(xcode_config.sdk_version_for_platform(platform))
    minimum = str(xcode_config.minimum_os_for_platform_type(platform_type))
    floor = deployment_floor(sdk)
    if version_below(sdk_version, floor):
        fail("apple: Xcode {} ships the {} {} SDK, below {}".format(xcode, sdk, sdk_version, floor))
    if tooling.deployment and (version_below(tooling.deployment, minimum) or version_below(minimum, tooling.deployment)):
        fail("apple: provisioning sets deployment {} but this build uses {}; pass {}={}".format(
            tooling.deployment,
            minimum,
            MINIMUM_OS_FLAGS[sdk],
            tooling.deployment,
        ))
    if version_below(sdk_version, minimum):
        fail("apple: deployment {} is above the {} {} SDK in Xcode {}".format(minimum, sdk, sdk_version, xcode))
    cc_toolchain = find_cc_toolchain(ctx, mandatory = False)
    if cc_toolchain == None:
        fail("apple: no C/C++ toolchain targets {} from this executor; the {} route needs a {} execution platform with Xcode".format(
            sdk,
            tooling.route,
            tooling.host,
        ))
    features = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    compiler = cc_common.get_tool_for_action(
        feature_configuration = features,
        action_name = C_COMPILE_ACTION_NAME,
    )

    report = ctx.actions.declare_file(ctx.label.name + ".txt")
    args = ctx.actions.args()
    args.add(report)
    args.add_all([
        tooling.route,
        tooling.host,
        xcode,
        sdk,
        sdk_version,
        minimum,
        tooling.macos_version,
        compiler,
        tooling.endpoint,
    ])
    env = {"PATH": _PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    env.update(apple_common.target_apple_env(xcode_config, platform))
    requirements = dict(xcode_config.execution_info())
    requirements["no-cache"] = "1"
    if tooling.route == MANAGED_ROUTE:
        requirements["no-local"] = "1"
    ctx.actions.run(
        executable = ctx.executable._probe,
        arguments = [args],
        inputs = cc_toolchain.all_files,
        outputs = [report],
        env = env,
        execution_requirements = requirements,
        mnemonic = "AppleToolingProbe",
        progress_message = "Probing Xcode {} and the {} SDK for %{{label}}".format(xcode, sdk),
    )
    return [DefaultInfo(files = depset([report]))]

apple_toolchain_probe = rule(
    implementation = _apple_toolchain_probe_impl,
    doc = "Records the executor's real Xcode, SDK, compiler, and macOS identities, failing on any mismatch.",
    attrs = dict(apple_support.action_required_attrs(), **{
        "provisioning": attr.label(
            doc = "The apple_provisioning selection to qualify.",
            mandatory = True,
            providers = [AppleToolingInfo],
        ),
        "_ios": attr.label(default = "@platforms//os:ios"),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_probe": attr.label(
            default = ":probe.sh",
            allow_single_file = True,
            cfg = "exec",
            executable = True,
        ),
        "_simulator": attr.label(default = "@apple_support//constraints:simulator"),
    }),
    fragments = ["apple", "cpp"],
    toolchains = use_cc_toolchain(mandatory = False),
)
