"""Rust iOS device and simulator static libraries, one checked cell each."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", "rust_static_library")
load("//apple/bundle:defs.bzl", "archives", "check_cell", "display", "resolved_xcode")
load(
    "//apple/provisioning:defs.bzl",
    _XCODE_LINEAGE = "XCODE_LINEAGE",
    _deployment_floor = "deployment_floor",
    _version_below = "version_below",
)
load(":cell.bzl", "APPLE_HOSTS", "CELL_ATTRS", "MACH_O_PLATFORMS", "ios_platform", "target_cell")

DEVICE_ENV = "device"

SIMULATOR_ENV = "simulator"

_DEVICE_ARCH = "arm64"

_SIMULATOR_ARCHES = ["arm64", "x86_64"]

_ENVIRONMENTS = {
    "device": struct(
        arches = [_DEVICE_ARCH],
        sdk = "iphoneos",
        triples = {"arm64": "aarch64-apple-ios"},
    ),
    "simulator": struct(
        arches = _SIMULATOR_ARCHES,
        sdk = "iphonesimulator",
        triples = {
            "arm64": "aarch64-apple-ios-sim",
            "x86_64": "x86_64-apple-ios",
        },
    ),
}

_SDK_CEILINGS = {
    "iphoneos": "18.0",
    "iphonesimulator": "18.0",
}

_DIGITS = "0123456789"

_PLATFORMS = {
    "device/arm64": Label("@apple_support//platforms:ios_arm64"),
    "simulator/arm64": Label("@apple_support//platforms:ios_sim_arm64"),
    "simulator/x86_64": Label("@apple_support//platforms:ios_x86_64"),
}

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

def environments():
    """Returns the supported iOS library environment names."""
    return sorted(_ENVIRONMENTS.keys())

def arches(environment):
    """Returns the supported architectures for one environment."""
    return list(_ENVIRONMENTS[environment].arches) if environment in _ENVIRONMENTS else []

def sdk_for(environment):
    """Returns the SDK identity for one environment, or empty."""
    return _ENVIRONMENTS[environment].sdk if environment in _ENVIRONMENTS else ""

def triple_for(environment, arch):
    """Returns the Rust target triple for one environment and arch, or empty."""
    if environment not in _ENVIRONMENTS:
        return ""
    triples = _ENVIRONMENTS[environment].triples
    return triples[arch] if arch in triples else ""

def ceiling_for(sdk):
    """Returns the Xcode 16 deployment ceiling for one SDK, or empty."""
    return _SDK_CEILINGS[sdk] if sdk in _SDK_CEILINGS else ""

def environment_errors(environment):
    """Returns one error string when the library environment leaves the set."""
    if environment == "":
        return ["apple_ios: select one environment: want " + ", ".join(environments())]
    if environment not in _ENVIRONMENTS:
        return ["apple_ios: unknown environment '" + environment + "': want " + ", ".join(environments())]
    return []

def arch_errors(environment, arch):
    """Returns one error string per rejected environment and arch pairing."""
    if environment not in _ENVIRONMENTS:
        return []
    if arch == "":
        return ["apple_ios: select one arch for '" + environment + "': want " + ", ".join(arches(environment))]
    if arch in _ENVIRONMENTS[environment].arches:
        return []
    if environment == DEVICE_ENV:
        return ["apple_ios: device builds need arch 'arm64'; '" + arch + "' runs on the simulator"]
    return ["apple_ios: arch '" + arch + "' is outside " + environment + ": want " + ", ".join(arches(environment))]

def triple_errors(environment, arch, triple):
    """Returns one error string when the Rust triple disagrees with the cell."""
    if triple == "":
        return []
    if environment not in _ENVIRONMENTS or arch not in _ENVIRONMENTS[environment].arches:
        return []
    want = triple_for(environment, arch)
    if triple == want:
        return []
    return ["apple_ios: triple '" + triple + "' disagrees with " + environment + "/" + arch + ": want '" + want + "'"]

def sdk_errors(environment, sdk):
    """Returns one error string when the SDK disagrees with the environment."""
    if sdk == "":
        return []
    if environment not in _ENVIRONMENTS:
        return []
    want = sdk_for(environment)
    if sdk == want:
        return []
    return ["apple_ios: sdk '" + sdk + "' disagrees with '" + environment + "': want '" + want + "'"]

def _parts(version):
    numbers = []
    for part in version.split("."):
        if part == "":
            return []
        for char in part.elems():
            if char not in _DIGITS:
                return []
        numbers.append(int(part))
    return numbers

def _above(version, ceiling):
    left = _parts(version)
    right = _parts(ceiling)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first > second
    return False

def _below(version, minimum):
    left = _parts(version)
    right = _parts(minimum)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first < second
    return False

def xcode_errors(version):
    """Returns one error string when the Xcode version leaves the lineage."""
    if version == "":
        return []
    if len(_parts(version)) == 0:
        return ["apple_ios: xcode_version '" + version + "' is not a dotted version"]
    if not version.startswith(_XCODE_LINEAGE):
        return ["apple_ios: xcode '" + version + "' is outside " + _XCODE_LINEAGE.rstrip(".")]
    return []

def deployment_errors(environment, deployment):
    """Returns one error string per rejected deployment target selection."""
    if deployment == "":
        return []
    if len(_parts(deployment)) == 0:
        return ["apple_ios: deployment_target '" + deployment + "' is not a dotted version"]
    sdk = sdk_for(environment)
    floor = _deployment_floor(sdk) if sdk != "" else ""
    errors = []
    if floor != "" and _below(deployment, floor):
        errors.append("apple_ios: deployment_target '" + deployment + "' is below " + floor)
    ceiling = ceiling_for(sdk)
    if ceiling != "" and _above(deployment, ceiling):
        errors.append("apple_ios: deployment_target '" + deployment + "' is above the Xcode 16 '" + sdk + "' ceiling " + ceiling)
    return errors

def framework_errors(frameworks):
    """Returns one error string per empty linked framework name."""
    errors = []
    for framework in frameworks:
        if framework == "":
            errors.append("apple_ios: framework names must not be empty")
    return errors

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
    changed[_MINIMUM_OS_SETTING] = deployment if deployment != "" else _deployment_floor(sdk_for(environment))
    if xcode != "":
        changed[_XCODE_SETTING] = xcode
    return changed

def framework_linkopts(frameworks):
    """Returns one single-token link flag per linked Apple framework."""
    return ["-Wl,-framework," + framework for framework in frameworks]

def deployment_env(environment, deployment):
    """Returns the rustc environment that pins the iOS deployment target."""
    return {"IPHONEOS_DEPLOYMENT_TARGET": deployment if deployment != "" else _deployment_floor(sdk_for(environment))}

def default_config(environment):
    """Returns the default library configuration for one environment."""
    return struct(
        arch = "",
        deployment = "",
        environment = environment,
        crate_features = [],
        frameworks = [],
        rustc_flags = [],
        sdk = "",
        triple = "",
        xcode_version = "",
    )

def config_errors(config):
    """Returns one error string per rejected field of one library configuration."""
    errors = list(environment_errors(config.environment))
    errors.extend(arch_errors(config.environment, config.arch))
    errors.extend(triple_errors(config.environment, config.arch, config.triple))
    errors.extend(sdk_errors(config.environment, config.sdk))
    errors.extend(deployment_errors(config.environment, config.deployment))
    errors.extend(xcode_errors(config.xcode_version))
    errors.extend(framework_errors(config.frameworks))
    return errors

def _ios_lib_selection_impl(ctx):
    failures = config_errors(struct(
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
        "arch": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "environment": attr.string(mandatory = True),
        "crate_features": attr.string_list(mandatory = True),
        "frameworks": attr.string_list(mandatory = True),
        "rustc_flags": attr.string_list(mandatory = True),
        "sdk": attr.string(mandatory = True),
        "triple": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
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
    "arch": attr.string(mandatory = True),
    "crate_features": attr.string_list(),
    "deployment": attr.string(default = ""),
    "environment": attr.string(mandatory = True),
    "frameworks": attr.string_list(),
    "rustc_flags": attr.string_list(),
    "sdk": attr.string(default = ""),
    "triple": attr.string(default = ""),
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
    failures = config_errors(_config(ctx))
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
        sdk_for(ctx.attr.environment),
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
            triple = triple_for(ctx.attr.environment, ctx.attr.arch),
        ),
    ]

ios_lib_cell = rule(
    implementation = _ios_lib_cell_impl,
    doc = "Checks one Rust iOS static library in the configuration of its cell.",
    attrs = dict(CELL_ATTRS, **dict(_SELECTION_ATTRS, **{
        "hdrs": attr.label_list(allow_files = [".h"]),
        "library": attr.label(mandatory = True, providers = [CcInfo]),
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
        "arch": attr.string(mandatory = True),
        "cell": attr.label(
            doc = "The ios_lib_cell that checks the archive.",
            cfg = _cell_transition,
            mandatory = True,
            providers = [CcInfo, AppleIosLibInfo],
        ),
        "deployment": attr.string(default = ""),
        "environment": attr.string(mandatory = True),
        "xcode_version": attr.string(default = ""),
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
        environment = "",
        arch = "",
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
