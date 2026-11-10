"""Rust iOS device and simulator static libraries, one checked cell each."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", "rust_static_library")
load("//bundle:defs.bzl", "archives", "check_cell", "display", "resolved_xcode")
load("//provisioning:defs.bzl", _deployment_floor = "deployment_floor")
load("//provisioning:validate.bzl", _version_below = "provisioning_version_below")
load(":cell.bzl", "APPLE_HOSTS", "CELL_ATTRS", "MACH_O_PLATFORMS", "ios_platform", "target_cell")
load(":validate.bzl", _DEVICE_ENV = "DEVICE_ENV", "IOS_PLATFORMS", "ios_config_errors", "ios_sdk_for", "ios_triple_for")

DEVICE_ENV = _DEVICE_ENV

SIMULATOR_ENV = "simulator"

_PLATFORMS = IOS_PLATFORMS

_PLATFORMS_SETTING = "//command_line_option:platforms"

_PLATFORM_TYPE_SETTING = "//command_line_option:apple_platform_type"

_MINIMUM_OS_SETTING = "//command_line_option:ios_minimum_os"

_XCODE_SETTING = "//command_line_option:xcode_version"

CELL_SETTINGS = [
    _PLATFORMS_SETTING,
    _PLATFORM_TYPE_SETTING,
    _MINIMUM_OS_SETTING,
    _XCODE_SETTING,
]

def ios_platforms():
    """Returns the supported iOS platform names, such as device/arm64."""
    return sorted(_PLATFORMS.keys())

def platform_slice(platform):
    """Returns the environment/arch name of an iOS platform Label, or an empty string."""
    for key, label in _PLATFORMS.items():
        if label == platform:
            return key
    return ""

def ios_slice(platform):
    """Returns the environment/arch name of a platform label given to a macro, or an empty string."""
    if platform == None:
        return ""
    return platform_slice(native.package_relative_label(platform))

def split_platform(slice):
    """Returns the environment and arch of an environment/arch name, or empty strings."""
    if slice not in _PLATFORMS:
        return ("", "")
    parts = slice.split("/")
    return (parts[0], parts[1])

def ios_platform_errors(kind, environment):
    """Returns one error string when the platform label is not a supported iOS platform."""
    if environment != "":
        return []
    return [kind + ": platform takes one of " + ", ".join([str(_PLATFORMS[key]) for key in ios_platforms()])]

def _label(value):
    return value if type(value) == "Label" else Label(value)

def platform_for(environment, arch):
    """Returns the apple_support platform Label for one cell, or None."""
    return _PLATFORMS.get(environment + "/" + arch)

def cell_settings(settings, environment, arch, deployment, xcode):
    """Returns the build settings that build one cell, unchanged on an iOS platform."""
    unchanged = {key: settings[key] for key in CELL_SETTINGS}
    platform = platform_for(environment, arch)
    if platform == None:
        return unchanged
    current = settings[_PLATFORMS_SETTING]
    if len(current) > 0 and _label(current[0]) in _PLATFORMS.values():
        return unchanged
    changed = dict(unchanged)
    changed[_PLATFORMS_SETTING] = [platform]
    changed[_PLATFORM_TYPE_SETTING] = "ios"
    changed[_MINIMUM_OS_SETTING] = deployment if deployment != "" else _deployment_floor(ios_sdk_for(environment))
    if xcode != "":
        changed[_XCODE_SETTING] = xcode
    return changed

def framework_linkopts(frameworks):
    """Returns one single-token link flag per linked Apple framework."""
    return ["-Wl,-framework," + framework for framework in frameworks]

def deployment_env(environment, deployment):
    """Returns the rustc environment that pins the iOS deployment target."""
    return {"IPHONEOS_DEPLOYMENT_TARGET": deployment if deployment != "" else _deployment_floor(ios_sdk_for(environment))}

def _ios_lib_selection_impl(ctx):
    selected = ios_platform_errors("apple_ios_lib", ctx.attr.environment)
    if len(selected) > 0:
        fail("; ".join(selected))
    failures = ios_config_errors(struct(
        arch = ctx.attr.arch,
        deployment = ctx.attr.deployment,
        environment = ctx.attr.environment,
        crate_features = list(ctx.attr.crate_features),
        frameworks = list(ctx.attr.frameworks),
        rustc_flags = list(ctx.attr.rustc_flags),
        sdk = ctx.attr.sdk,
        triple = ctx.attr.triple,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_ios_lib_selection = rule(
    implementation = _ios_lib_selection_impl,
    doc = "Fails analysis when an iOS library selection is invalid.",
    attrs = {
        "arch": attr.string(
            doc = "Target architecture of the iOS library cell, such as arm64.",
            mandatory = True,
        ),
        "deployment": attr.string(
            doc = "Minimum iOS deployment version, or empty for the SDK floor.",
            mandatory = True,
        ),
        "environment": attr.string(
            doc = "iOS library environment, either device or simulator.",
            mandatory = True,
        ),
        "crate_features": attr.string_list(
            doc = "Cargo features enabled for the Rust iOS library.",
            mandatory = True,
        ),
        "frameworks": attr.string_list(
            doc = "Apple frameworks the Rust iOS library links against.",
            mandatory = True,
        ),
        "rustc_flags": attr.string_list(
            doc = "Extra flags passed to rustc for the library build.",
            mandatory = True,
        ),
        "sdk": attr.string(
            doc = "Apple SDK identity the library is checked against, such as iphoneos.",
            mandatory = True,
        ),
        "triple": attr.string(
            doc = "Rust target triple the library is expected to be built for.",
            mandatory = True,
        ),
        "xcode_version": attr.string(
            doc = "Xcode version the library is checked with, or empty when unset.",
            mandatory = True,
        ),
    },
)

AppleIosLibInfo = provider(
    doc = "One checked Rust iOS static library cell.",
    fields = {
        "archive": "The static archive File.",
        "cell": "The environment/arch cell, like 'simulator/arm64'.",
        "deployment": "The declared minimum iOS version, or empty for the floor.",
        "report": "The library check report File.",
        "triple": "The Rust target triple.",
    },
)

_SELECTION_ATTRS = {
    "arch": attr.string(
        doc = "Target architecture within the library environment, such as arm64.",
        mandatory = True,
    ),
    "crate_features": attr.string_list(doc = "Cargo features enabled for the Rust library."),
    "deployment": attr.string(
        doc = "Minimum deployment version, or empty for the platform floor.",
        default = "",
    ),
    "environment": attr.string(
        doc = "Library environment, either device or simulator.",
        mandatory = True,
    ),
    "frameworks": attr.string_list(doc = "Apple frameworks the Rust library links against."),
    "rustc_flags": attr.string_list(doc = "Extra flags passed to rustc for the library."),
    "sdk": attr.string(
        doc = "Apple SDK identity such as iphoneos, or empty when unset.",
        default = "",
    ),
    "triple": attr.string(
        doc = "Rust target triple for the library, or empty when unset.",
        default = "",
    ),
}

def _config(ctx):
    return struct(
        arch = ctx.attr.arch,
        deployment = ctx.attr.deployment,
        environment = ctx.attr.environment,
        crate_features = list(ctx.attr.crate_features),
        frameworks = list(ctx.attr.frameworks),
        rustc_flags = list(ctx.attr.rustc_flags),
        sdk = ctx.attr.sdk,
        triple = ctx.attr.triple,
        xcode_version = ctx.attr.xcode_version,
    )

def _ios_lib_cell_impl(ctx):
    failures = ios_config_errors(_config(ctx))
    if len(failures) > 0:
        fail("; ".join(failures))
    owner = display(ctx.label).removesuffix("_cell")
    want = ctx.attr.environment + "/" + ctx.attr.arch
    got = target_cell(ctx)
    if got != want:
        fail("apple_ios_lib: {} declares {} but this build targets {}".format(owner, want, got if got != "" else "no iOS platform"))
    cc_info = ctx.attr.library[CcInfo]
    found = archives(cc_info)
    if len(found) == 0:
        fail("apple_ios_lib: library {} links no static archive; pass a Rust static library".format(display(ctx.attr.library.label)))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_lib", "library")
    minimum = str(xcode_config.minimum_os_for_platform_type(apple_common.platform_type.ios))
    if ctx.attr.deployment != "" and _version_below(minimum, ctx.attr.deployment):
        fail("apple_ios_lib: {} deploys to {} but this build targets iOS {}".format(owner, ctx.attr.deployment, minimum))
    report = check_cell(
        ctx,
        "apple_ios_lib",
        want,
        ios_sdk_for(ctx.attr.environment),
        MACH_O_PLATFORMS[ctx.attr.environment],
        ios_platform(ctx.attr.environment),
        apple_common.platform_type.ios,
        "AppleIosLibraryCell",
        xcode_config,
        xcode,
        found,
    )
    root = ctx.label.workspace_root
    headers = CcInfo(
        compilation_context = cc_common.create_compilation_context(
            headers = depset(ctx.files.hdrs),
            quote_includes = depset([root] if root != "" else []),
        ),
        linking_context = cc_common.create_linking_context(
            linker_inputs = depset([cc_common.create_linker_input(
                owner = ctx.label,
                user_link_flags = framework_linkopts(ctx.attr.frameworks),
                additional_inputs = depset([report]),
            )]),
        ),
    )
    return [
        cc_common.merge_cc_infos(cc_infos = [cc_info, headers]),
        DefaultInfo(files = depset(found + [report])),
        AppleIosLibInfo(
            archive = found[0],
            cell = want,
            deployment = ctx.attr.deployment,
            report = report,
            triple = ios_triple_for(ctx.attr.environment, ctx.attr.arch),
        ),
    ]

ios_lib_cell = rule(
    implementation = _ios_lib_cell_impl,
    doc = "Checks one Rust iOS static library in the configuration of its cell.",
    attrs = dict(CELL_ATTRS, **dict(_SELECTION_ATTRS, **{
        "hdrs": attr.label_list(
            doc = "Public header files exported alongside the static library.",
            allow_files = [".h"],
        ),
        "library": attr.label(
            doc = "Rust static library target whose archive is checked.",
            mandatory = True,
            providers = [CcInfo],
        ),
    })),
    fragments = ["apple"],
)

def _cell_transition_impl(settings, attr):
    return cell_settings(settings, attr.environment, attr.arch, attr.deployment, attr.xcode_version)

_cell_transition = transition(
    implementation = _cell_transition_impl,
    inputs = CELL_SETTINGS,
    outputs = CELL_SETTINGS,
)

def _ios_lib_impl(ctx):
    cell = ctx.attr.cell[0] if type(ctx.attr.cell) == "list" else ctx.attr.cell
    return [
        cell[CcInfo],
        cell[DefaultInfo],
        cell[AppleIosLibInfo],
    ]

ios_lib = rule(
    implementation = _ios_lib_impl,
    doc = "Builds one Rust iOS static library cell, moving to the cell platform off iOS.",
    attrs = {
        "arch": attr.string(
            doc = "Target architecture of the library cell, such as arm64.",
            mandatory = True,
        ),
        "cell": attr.label(
            doc = "The ios_lib_cell that checks the archive.",
            cfg = _cell_transition,
            mandatory = True,
            providers = [CcInfo, AppleIosLibInfo],
        ),
        "deployment": attr.string(
            doc = "Minimum iOS deployment version, or empty for the floor.",
            default = "",
        ),
        "environment": attr.string(
            doc = "iOS library environment, either device or simulator.",
            mandatory = True,
        ),
        "xcode_version": attr.string(
            doc = "Xcode version selected for the cell build, or empty when unset.",
            default = "",
        ),
    },
)

def apple_ios_lib(
        name,
        srcs = [],
        deps = [],
        proc_macro_deps = [],
        crate_root = None,
        crate_name = None,
        edition = None,
        hdrs = [],
        platform = None,
        triple = "",
        sdk = "",
        deployment = "",
        xcode_version = "",
        crate_features = [],
        rustc_flags = [],
        frameworks = [],
        tags = [],
        testonly = False,
        visibility = None):
    """Builds one Rust static library for one iOS device or simulator cell and checks its Mach-O."""
    environment, arch = split_platform(ios_slice(platform))
    _ios_lib_selection(
        name = name + "_selection",
        arch = arch,
        crate_features = crate_features,
        deployment = deployment,
        environment = environment,
        frameworks = frameworks,
        rustc_flags = rustc_flags,
        sdk = sdk,
        triple = triple,
        xcode_version = xcode_version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    rust_static_library(
        name = name + "_rust",
        srcs = srcs,
        deps = deps,
        proc_macro_deps = proc_macro_deps,
        crate_root = crate_root,
        crate_name = crate_name if crate_name != None else name.replace("-", "_"),
        crate_features = crate_features,
        edition = edition,
        rustc_flags = rustc_flags,
        rustc_env = deployment_env(environment, deployment),
        tags = ["manual"],
        testonly = testonly,
        target_compatible_with = [Label("@platforms//os:ios")],
        visibility = ["//visibility:private"],
    )
    ios_lib_cell(
        name = name + "_cell",
        arch = arch,
        crate_features = crate_features,
        deployment = deployment,
        environment = environment,
        frameworks = frameworks,
        hdrs = hdrs,
        library = ":" + name + "_rust",
        rustc_flags = rustc_flags,
        sdk = sdk,
        triple = triple,
        xcode_version = xcode_version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    ios_lib(
        name = name,
        arch = arch,
        cell = ":" + name + "_cell",
        deployment = deployment,
        environment = environment,
        xcode_version = xcode_version,
        tags = tags,
        testonly = testonly,
        target_compatible_with = APPLE_HOSTS,
        visibility = visibility,
    )
