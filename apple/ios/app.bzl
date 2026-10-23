"""Simulator applications and XCFrameworks over Rust static libraries."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("@rules_apple//apple:apple.bzl", "apple_xcframework")
load("@rules_apple//apple:ios.bzl", "ios_application", "ios_unit_test")
load("@rules_apple//apple:versioning.bzl", "apple_bundle_version")
load("@rules_apple//apple/testing/default_runner:ios_xctestrun_runner.bzl", "ios_xctestrun_runner")
load("@rules_cc//cc:objc_library.bzl", "objc_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//apple/provisioning:defs.bzl", "version_below", "version_matches")
load(
    ":defs.bzl",
    "arch_errors",
    "arches",
    "deployment_errors",
    "environment_errors",
    "environments",
    "sdk_for",
    "xcode_errors",
)

_ID_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"

_DIGITS = "0123456789"

_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"

_MULTI_CPUS = {
    "device/arm64": "arm64",
    "simulator/arm64": "sim_arm64",
    "simulator/x86_64": "x86_64",
}

_COMPILED_RESOURCES = ["storyboard", "xcdatamodel", "xcmappingmodel", "xcstrings", "xib"]

_GROUPED_RESOURCES = (".atlas", ".bundle", ".lproj", ".scnassets", ".xcassets", ".xcdatamodeld")

_MACH_O_PLATFORMS = {
    "device": "2",
    "simulator": "7",
}

_APPLE_HOSTS = select({
    Label("@platforms//os:ios"): [],
    Label("@platforms//os:macos"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

_INFO_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>$(EXECUTABLE_NAME)</string>
  <key>CFBundleIdentifier</key>
  <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$(PRODUCT_NAME)</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>LSRequiresIPhoneOS</key>
  <true/>
  <key>UILaunchScreen</key>
  <dict/>
</dict>
</plist>
"""

_FRAMEWORK_INFO_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>$(EXECUTABLE_NAME)</string>
  <key>CFBundleIdentifier</key>
  <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$(PRODUCT_NAME)</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
</dict>
</plist>
"""

def bundle_id_errors(kind, bundle_id):
    """Returns one error string when the bundle identifier leaves reverse-DNS."""
    if bundle_id == "":
        return [kind + ": bundle_id must not be empty"]
    parts = bundle_id.split(".")
    if len(parts) < 2:
        return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
    for part in parts:
        if part == "":
            return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
        for char in part.elems():
            if char not in _ID_CHARS:
                return [kind + ": bundle_id '" + bundle_id + "' uses '" + char + "': want letters, digits, '-', '_', '.'"]
    return []

def bundle_version_errors(kind, version):
    """Returns one error string when the bundle version leaves dotted digits."""
    if version == "":
        return [kind + ": bundle version must not be empty"]
    for part in version.split("."):
        if part == "":
            return [kind + ": bundle version '" + version + "' is not a dotted version"]
        for char in part.elems():
            if char not in _DIGITS:
                return [kind + ": bundle version '" + version + "' is not a dotted version"]
    return []

def app_resource_errors(resources):
    """Returns one error string per empty app resource name."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_app: resource names must not be empty")
    return errors

def entitlement_errors(entitlements):
    """Returns one error string when the entitlements path leaves its contract."""
    if entitlements == "":
        return []
    if entitlements.endswith(".entitlements"):
        return []
    return ["apple_app: entitlements '" + entitlements + "' must be an .entitlements file"]

def signing_errors(environment, provisioning_profile):
    """Returns one error string per rejected app signing selection."""
    if environment == "device":
        if provisioning_profile == "":
            return ["apple_app: device distribution needs provisioning_profile; credentials travel by file only"]
        return []
    if provisioning_profile != "":
        return ["apple_app: simulator builds sign ad hoc; leave provisioning_profile empty"]
    return []

def entry_errors(srcs):
    """Returns one error string when the app has no native entry source."""
    if len(srcs) == 0:
        return ["apple_app: srcs needs the native entry point that calls UIApplicationMain"]
    return []

def default_app(environment):
    """Returns the default simulator application composition for one environment."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        environment = environment,
        export_ipa = False,
        frameworks = [],
        library = "",
        provisioning_profile = "",
        resources = [],
        srcs = [],
        version = "",
        xcode_version = "",
    )

def app_errors(config):
    """Returns one error string per rejected field of one app composition."""
    errors = list(environment_errors(config.environment))
    errors.extend(arch_errors(config.environment, config.arch))
    if config.deployment == "":
        errors.append("apple_app: deployment names the minimum iOS version")
    errors.extend(deployment_errors(config.environment, config.deployment))
    errors.extend(xcode_errors(config.xcode_version))
    errors.extend(bundle_id_errors("apple_app", config.bundle_id))
    errors.extend(bundle_version_errors("apple_app", config.version))
    errors.extend(app_resource_errors(config.resources))
    errors.extend(entitlement_errors(config.entitlements))
    errors.extend(signing_errors(config.environment, config.provisioning_profile))
    errors.extend(entry_errors(config.srcs))
    if config.library == "" and len(config.frameworks) == 0:
        errors.append("apple_app: app composes one qualified Rust library cell in library or XCFramework imports in frameworks")
    if config.export_ipa:
        errors.append("apple_app: export is a distinct explicitly applied operation; Check never exports an IPA")
    return errors

def test_errors(device_type, os_version, deployment):
    """Returns one error string per rejected simulator test selection."""
    errors = []
    if device_type == "":
        errors.append("apple_app_test: device_type names the simulator device, like 'iPhone 16'")
    if os_version == "":
        errors.append("apple_app_test: os_version names the installed simulator runtime, like '18.5'")
    else:
        for part in os_version.split("."):
            if part == "" or not part.isdigit():
                errors.append("apple_app_test: os_version '" + os_version + "' is not a dotted version")
                break
    if deployment == "":
        errors.append("apple_app_test: deployment names the minimum iOS version of the host app")
    else:
        errors.extend(deployment_errors("simulator", deployment))
        if len(errors) == 0 and version_below(os_version, deployment):
            errors.append("apple_app_test: simulator runtime iOS " + os_version + " is below the app deployment " + deployment)
    return errors

def _app_selection_impl(ctx):
    failures = app_errors(struct(
        arch = ctx.attr.arch,
        bundle_id = ctx.attr.bundle_id,
        deployment = ctx.attr.deployment,
        entitlements = ctx.attr.entitlements,
        environment = ctx.attr.environment,
        export_ipa = ctx.attr.export_ipa,
        frameworks = list(ctx.attr.framework_imports),
        library = ctx.attr.library,
        provisioning_profile = ctx.attr.provisioning_profile,
        resources = list(ctx.attr.resources),
        srcs = list(ctx.attr.srcs),
        version = ctx.attr.version,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return [CcInfo()]

_app_selection = rule(
    implementation = _app_selection_impl,
    attrs = {
        "arch": attr.string(mandatory = True),
        "bundle_id": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "entitlements": attr.string(mandatory = True),
        "environment": attr.string(mandatory = True),
        "export_ipa": attr.bool(mandatory = True),
        "framework_imports": attr.string_list(mandatory = True),
        "library": attr.string(mandatory = True),
        "provisioning_profile": attr.string(mandatory = True),
        "resources": attr.string_list(mandatory = True),
        "srcs": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
    },
)

def _has(ctx, constraint):
    return ctx.target_platform_has_constraint(constraint[platform_common.ConstraintValueInfo])

def _target_cell(ctx):
    if not _has(ctx, ctx.attr._ios):
        return ""
    environment = "simulator" if _has(ctx, ctx.attr._simulator) else "device"
    if _has(ctx, ctx.attr._arm64):
        return environment + "/arm64"
    if _has(ctx, ctx.attr._x86_64):
        return environment + "/x86_64"
    return environment + "/unknown"

def _display(label):
    if label.repo_name == "":
        return "//{}:{}".format(label.package, label.name)
    return str(label)

def _archives(cc_info):
    archives = []
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        for library in linker_input.libraries:
            archive = library.static_library or library.pic_static_library
            if archive != None:
                archives.append(archive)
    return archives

def _framework_binaries(cc_info):
    binaries = []
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        for library in linker_input.libraries:
            if library.dynamic_library != None:
                binaries.append(library.dynamic_library)
    return binaries

def _resolved_xcode(ctx, kind, owner):
    xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]
    if xcode_config.xcode_version() == None:
        fail("{}: no Xcode resolved for this build; building the {} needs a macOS host with Xcode".format(kind, owner))
    xcode = str(xcode_config.xcode_version())
    if ctx.attr.xcode_version and not version_matches(ctx.attr.xcode_version, xcode):
        fail("{}: {} selects Xcode {} but Bazel resolved {}; pass --xcode_version={}".format(
            kind,
            owner,
            ctx.attr.xcode_version,
            xcode,
            ctx.attr.xcode_version,
        ))
    return xcode_config, xcode

def _check_cell(ctx, kind, cell, xcode_config, xcode, inputs):
    environment, arch = cell.split("/")
    platform = apple_common.platform.ios_simulator if environment == "simulator" else apple_common.platform.ios_device
    deployment = str(xcode_config.minimum_os_for_platform_type(apple_common.platform_type.ios))
    report = ctx.actions.declare_file(ctx.label.name + ".txt")
    args = ctx.actions.args()
    args.add(kind)
    args.add(report)
    args.add_all([
        cell,
        sdk_for(environment),
        _MACH_O_PLATFORMS[environment],
        arch,
        deployment,
        xcode,
        str(xcode_config.sdk_version_for_platform(platform)),
    ])
    args.add_all(inputs)
    env = {"PATH": _PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    env.update(apple_common.target_apple_env(xcode_config, platform))
    ctx.actions.run(
        executable = ctx.executable._check,
        arguments = [args],
        inputs = inputs,
        outputs = [report],
        env = env,
        execution_requirements = xcode_config.execution_info(),
        mnemonic = "AppleIosLibraryCell",
        progress_message = "Checking the {} library cell of %{{label}}".format(cell),
    )
    return report

def _library_cell_impl(ctx):
    errors = environment_errors(ctx.attr.environment) + arch_errors(ctx.attr.environment, ctx.attr.arch)
    if len(errors) > 0:
        fail("; ".join(errors))
    want = ctx.attr.environment + "/" + ctx.attr.arch
    got = _target_cell(ctx)
    if got == "":
        fail("apple_ios_app: library cell {} builds for no iOS platform here; build the app target so rules_apple selects the cell".format(want))
    if got != want:
        fail("apple_ios_app: app declares {} but this build targets {}; pass --ios_multi_cpus={}".format(
            want,
            got,
            _MULTI_CPUS[want],
        ))
    xcode_config, xcode = _resolved_xcode(ctx, "apple_ios_app", "app")
    inputs = []
    cc_infos = []
    if ctx.attr.library != None:
        cc_info = ctx.attr.library[CcInfo]
        archives = _archives(cc_info)
        if len(archives) == 0:
            fail("apple_ios_app: library {} links no static archive; pass a Rust static library".format(_display(ctx.attr.library.label)))
        inputs.extend(archives)
        cc_infos.append(cc_info)
    for framework in ctx.attr.frameworks:
        binaries = _framework_binaries(framework[CcInfo])
        if len(binaries) == 0:
            fail("apple_ios_app: framework {} links no framework binary; pass an apple_dynamic_xcframework_import".format(_display(framework.label)))
        inputs.extend(binaries)
    report = _check_cell(ctx, "apple_ios_app", want, xcode_config, xcode, inputs)
    checked = CcInfo(linking_context = cc_common.create_linking_context(
        linker_inputs = depset([cc_common.create_linker_input(
            owner = ctx.label,
            additional_inputs = depset([report]),
        )]),
    ))
    return [
        cc_common.merge_cc_infos(cc_infos = cc_infos + [checked]),
        DefaultInfo(files = depset([report])),
    ]

_CELL_ATTRS = {
    "xcode_version": attr.string(default = ""),
    "_arm64": attr.label(default = "@platforms//cpu:arm64"),
    "_check": attr.label(
        default = ":library_check.sh",
        allow_single_file = True,
        cfg = "exec",
        executable = True,
    ),
    "_ios": attr.label(default = "@platforms//os:ios"),
    "_simulator": attr.label(default = "@apple_support//constraints:simulator"),
    "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
    "_xcode_config": attr.label(
        default = configuration_field(
            name = "xcode_config_label",
            fragment = "apple",
        ),
    ),
}

_library_cell = rule(
    implementation = _library_cell_impl,
    attrs = dict(_CELL_ATTRS, **{
        "arch": attr.string(mandatory = True),
        "environment": attr.string(mandatory = True),
        "frameworks": attr.label_list(providers = [CcInfo]),
        "library": attr.label(providers = [CcInfo]),
    }),
    fragments = ["apple"],
)

def _forced(ctx, cc_info):
    inputs = []
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        libraries = []
        for library in linker_input.libraries:
            archive = library.static_library or library.pic_static_library
            if archive != None and not library.alwayslink:
                library = cc_common.create_library_to_link(
                    actions = ctx.actions,
                    static_library = archive,
                    alwayslink = True,
                )
            libraries.append(library)
        inputs.append(cc_common.create_linker_input(
            owner = linker_input.owner,
            libraries = depset(libraries),
            user_link_flags = linker_input.user_link_flags,
            additional_inputs = depset(linker_input.additional_inputs),
        ))
    return CcInfo(
        compilation_context = cc_info.compilation_context,
        linking_context = cc_common.create_linking_context(linker_inputs = depset(inputs)),
    )

def _framework_cell_impl(ctx):
    got = _target_cell(ctx)
    if got == "":
        fail("apple_ios_framework: framework slices build for no iOS platform here; build the framework target so rules_apple selects each slice")
    if got not in ctx.attr.slices:
        fail("apple_ios_framework: this build targets {}, outside the framework slices {}".format(got, ", ".join(ctx.attr.slices)))
    xcode_config, xcode = _resolved_xcode(ctx, "apple_ios_framework", "framework")
    cc_info = ctx.attr.library[CcInfo]
    archives = _archives(cc_info)
    if len(archives) == 0:
        fail("apple_ios_framework: library {} links no static archive; pass a Rust static library".format(_display(ctx.attr.library.label)))
    report = _check_cell(ctx, "apple_ios_framework", got, xcode_config, xcode, archives)
    checked = CcInfo(linking_context = cc_common.create_linking_context(
        linker_inputs = depset([cc_common.create_linker_input(
            owner = ctx.label,
            additional_inputs = depset([report]),
        )]),
    ))
    return [
        cc_common.merge_cc_infos(cc_infos = [_forced(ctx, cc_info), checked]),
        DefaultInfo(files = depset([report])),
    ]

_framework_cell = rule(
    implementation = _framework_cell_impl,
    attrs = dict(_CELL_ATTRS, **{
        "library": attr.label(mandatory = True, providers = [CcInfo]),
        "slices": attr.string_list(mandatory = True),
    }),
    fragments = ["apple"],
)

def _label_text(value):
    return "" if value == None else str(value)

def apple_ios_app(
        name,
        library = None,
        frameworks = [],
        srcs = [],
        hdrs = [],
        deps = [],
        environment = "",
        arch = "arm64",
        bundle_id = "",
        version = "",
        deployment = "",
        xcode_version = "",
        resources = [],
        infoplists = [],
        families = ["iphone"],
        entitlements = None,
        provisioning_profile = None,
        export_ipa = False,
        tags = [],
        testonly = False,
        visibility = None):
    """Builds one upstream iOS application bundle over a Rust library cell or XCFramework imports."""
    composed = library != None or len(frameworks) > 0
    _app_selection(
        name = name + "_selection" if composed else name,
        arch = arch,
        bundle_id = bundle_id,
        deployment = deployment,
        entitlements = _label_text(entitlements),
        environment = environment,
        export_ipa = export_ipa,
        framework_imports = [_label_text(framework) for framework in frameworks],
        library = _label_text(library),
        provisioning_profile = _label_text(provisioning_profile),
        resources = [_label_text(resource) for resource in resources],
        srcs = [_label_text(src) for src in srcs],
        version = version,
        xcode_version = xcode_version,
        tags = ["manual"] if composed else tags,
        testonly = testonly,
        visibility = ["//visibility:private"] if composed else visibility,
    )
    if not composed:
        return
    _library_cell(
        name = name + "_library",
        arch = arch,
        environment = environment,
        frameworks = frameworks,
        library = library,
        xcode_version = xcode_version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    objc_library(
        name = name + "_entry",
        srcs = srcs,
        hdrs = hdrs,
        deps = [
            ":" + name + "_selection",
            ":" + name + "_library",
        ] + frameworks + deps,
        tags = ["manual"],
        testonly = testonly,
        target_compatible_with = _APPLE_HOSTS,
        visibility = ["//visibility:private"],
    )
    apple_bundle_version(
        name = name + "_version",
        build_version = version,
        short_version_string = version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    if len(infoplists) == 0:
        write_file(
            name = name + "_infoplist",
            out = name + "_Info.plist",
            content = [_INFO_PLIST],
            tags = ["manual"],
            testonly = testonly,
            visibility = ["//visibility:private"],
        )
        infoplists = [":" + name + "_infoplist"]
    ios_application(
        name = name,
        bundle_id = bundle_id,
        entitlements = entitlements,
        families = families,
        infoplists = infoplists,
        minimum_os_version = deployment,
        provisioning_profile = provisioning_profile,
        resources = resources,
        version = ":" + name + "_version",
        deps = [":" + name + "_entry"],
        tags = tags,
        testonly = testonly,
        target_compatible_with = _APPLE_HOSTS,
        visibility = visibility,
    )

def _test_selection_impl(ctx):
    failures = test_errors(ctx.attr.device_type, ctx.attr.os_version, ctx.attr.deployment)
    if len(failures) > 0:
        fail("; ".join(failures))
    return [CcInfo()]

_test_selection = rule(
    implementation = _test_selection_impl,
    attrs = {
        "deployment": attr.string(mandatory = True),
        "device_type": attr.string(mandatory = True),
        "os_version": attr.string(mandatory = True),
    },
)

def apple_ios_app_test(
        name,
        app,
        srcs,
        deployment = "",
        device_type = "",
        os_version = "",
        deps = [],
        tags = [],
        visibility = None,
        **kwargs):
    """Runs XCTest cases inside one app on a declared simulator device and runtime."""
    _test_selection(
        name = name + "_selection",
        deployment = deployment,
        device_type = device_type,
        os_version = os_version,
        tags = ["manual"],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    objc_library(
        name = name + "_lib",
        srcs = srcs,
        deps = [":" + name + "_selection"] + deps,
        tags = ["manual"],
        testonly = True,
        target_compatible_with = _APPLE_HOSTS,
        visibility = ["//visibility:private"],
    )
    ios_xctestrun_runner(
        name = name + "_runner",
        clean_up_simulator_action = Label("//apple/ios:delete_test_simulator"),
        create_simulator_action = Label("//apple/ios:create_test_simulator"),
        device_type = device_type,
        os_version = os_version,
        tags = ["manual"],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    ios_unit_test(
        name = name,
        minimum_os_version = deployment,
        runner = ":" + name + "_runner",
        test_host = app,
        deps = [":" + name + "_lib"],
        tags = sorted(set(tags + ["no-sandbox"])),
        target_compatible_with = _APPLE_HOSTS,
        visibility = visibility,
        **kwargs
    )

def slice_errors(slices):
    """Returns one error string per rejected framework slice selection."""
    if len(slices) == 0:
        return ["apple_framework: framework needs at least one 'environment/arch' slice"]
    errors = []
    seen = {}
    has_device = False
    has_simulator = False
    for slice in slices:
        if slice in seen:
            errors.append("apple_framework: duplicate slice '" + slice + "'")
            continue
        seen[slice] = True
        parts = slice.split("/")
        if len(parts) != 2 or parts[0] == "" or parts[1] == "":
            errors.append("apple_framework: slice '" + slice + "' is not 'environment/arch'")
            continue
        environment = parts[0]
        arch = parts[1]
        if environment not in environments():
            errors.append("apple_framework: unknown environment '" + environment + "' in slice '" + slice + "': want " + ", ".join(environments()))
            continue
        if arch not in arches(environment):
            errors.append("apple_framework: arch '" + arch + "' is outside " + environment + " in slice '" + slice + "': want " + ", ".join(arches(environment)))
            continue
        if environment == "device":
            has_device = True
        else:
            has_simulator = True
    if not has_device:
        errors.append("apple_framework: framework needs the device/arm64 slice")
    if not has_simulator:
        errors.append("apple_framework: framework needs at least one simulator slice")
    return errors

def framework_resource_errors(resources):
    """Returns one error string per empty or absolute framework resource path."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_framework: resource names must not be empty")
        elif resource.startswith("/"):
            errors.append("apple_framework: resource '" + resource + "' must be a relative workspace path")
    return errors

def framework_header_errors(hdrs, module):
    """Returns one error string per missing, non-header, or umbrella-clashing public header."""
    if len(hdrs) == 0:
        return ["apple_framework: hdrs needs at least one public header for the framework module map"]
    errors = []
    umbrella = module + ".h"
    for header in hdrs:
        if not header.endswith(".h"):
            errors.append("apple_framework: public header '" + header + "' must be a .h file")
        elif header.split(":")[-1].split("/")[-1].lower() == umbrella.lower():
            errors.append("apple_framework: public header '" + header + "' clashes with the umbrella header " + umbrella)
    return errors

def default_framework():
    """Returns the default embeddable framework composition."""
    return struct(
        bundle_id = "",
        bundle_name = "",
        deployment = "",
        hdrs = [],
        library = "",
        resources = [],
        slices = [],
        version = "",
        xcode_version = "",
    )

def framework_errors(config):
    """Returns one error string per rejected field of one framework composition."""
    errors = list(bundle_id_errors("apple_framework", config.bundle_id))
    errors.extend(bundle_version_errors("apple_framework", config.version))
    errors.extend(slice_errors(config.slices))
    errors.extend(framework_resource_errors(config.resources))
    errors.extend(framework_header_errors(config.hdrs, config.bundle_name))
    if config.deployment == "":
        errors.append("apple_framework: deployment names the minimum iOS version")
    errors.extend(deployment_errors("device", config.deployment))
    errors.extend(xcode_errors(config.xcode_version))
    if config.library == "":
        errors.append("apple_framework: framework composes one Rust static library in library")
    return errors

def _framework_selection_impl(ctx):
    failures = framework_errors(struct(
        bundle_id = ctx.attr.bundle_id,
        bundle_name = ctx.attr.bundle_name,
        deployment = ctx.attr.deployment,
        hdrs = list(ctx.attr.hdrs),
        library = ctx.attr.library,
        resources = list(ctx.attr.resources),
        slices = list(ctx.attr.slices),
        version = ctx.attr.version,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_framework_selection = rule(
    implementation = _framework_selection_impl,
    attrs = {
        "bundle_id": attr.string(mandatory = True),
        "bundle_name": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "hdrs": attr.string_list(mandatory = True),
        "library": attr.string(mandatory = True),
        "resources": attr.string_list(mandatory = True),
        "slices": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
    },
)

def _flat_resources(files):
    names = []
    for file in files:
        if file.extension in _COMPILED_RESOURCES:
            continue
        if any([part.endswith(_GROUPED_RESOURCES) for part in file.dirname.split("/")]):
            continue
        if file.basename not in names:
            names.append(file.basename)
    return sorted(names)

def _xcframework_check_impl(ctx):
    archives = [file for file in ctx.attr.xcframework[DefaultInfo].files.to_list() if file.basename.endswith(".xcframework.zip")]
    if len(archives) != 1:
        fail("apple_ios_framework: {} built {} .xcframework.zip outputs, want exactly one".format(
            _display(ctx.attr.xcframework.label),
            len(archives),
        ))
    xcode_config, xcode = _resolved_xcode(ctx, "apple_ios_framework", "framework")
    report = ctx.actions.declare_file(ctx.label.name + "_xcframework.txt")
    out = ctx.actions.declare_file(ctx.label.name + ".xcframework.zip")
    args = ctx.actions.args()
    args.add(report)
    args.add(out)
    args.add(archives[0])
    args.add_all([
        ctx.attr.bundle_name,
        ctx.attr.bundle_id,
        ctx.attr.version,
        ctx.attr.deployment,
        xcode,
        ",".join(ctx.attr.slices),
        ",".join(_flat_resources(ctx.files.resources)),
    ])
    args.add_all(sorted([header.basename for header in ctx.files.hdrs]))
    env = {"PATH": _PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    ctx.actions.run(
        executable = ctx.executable._check,
        arguments = [args],
        inputs = archives,
        outputs = [report, out],
        env = env,
        execution_requirements = xcode_config.execution_info(),
        mnemonic = "AppleXcframeworkCheck",
        progress_message = "Checking the XCFramework slices of %{label}",
    )
    return [DefaultInfo(files = depset([out, report]))]

_xcframework_check = rule(
    implementation = _xcframework_check_impl,
    attrs = {
        "bundle_id": attr.string(mandatory = True),
        "bundle_name": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "hdrs": attr.label_list(allow_files = [".h"]),
        "resources": attr.label_list(allow_files = True),
        "selection": attr.label(mandatory = True),
        "slices": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
        "xcframework": attr.label(mandatory = True),
        "xcode_version": attr.string(default = ""),
        "_check": attr.label(
            default = ":xcframework_check.sh",
            allow_single_file = True,
            cfg = "exec",
            executable = True,
        ),
        "_xcode_config": attr.label(
            default = configuration_field(
                name = "xcode_config_label",
                fragment = "apple",
            ),
        ),
    },
    fragments = ["apple"],
)

def _slice_dict(slices):
    ios = {}
    for slice in slices:
        parts = slice.split("/")
        if len(parts) == 2 and parts[1] not in ios.get(parts[0], []):
            ios[parts[0]] = ios.get(parts[0], []) + [parts[1]]
    return ios

def apple_ios_framework(
        name,
        library = None,
        hdrs = [],
        deps = [],
        bundle_id = "",
        bundle_name = "",
        version = "",
        deployment = "",
        xcode_version = "",
        slices = [],
        resources = [],
        infoplists = [],
        tags = [],
        testonly = False,
        visibility = None):
    """Builds and checks one XCFramework over a Rust static library and public headers."""
    module = bundle_name or name
    _framework_selection(
        name = name + "_selection" if library != None else name,
        bundle_id = bundle_id,
        bundle_name = module,
        deployment = deployment,
        hdrs = [_label_text(header) for header in hdrs],
        library = _label_text(library),
        resources = [_label_text(resource) for resource in resources],
        slices = slices,
        version = version,
        xcode_version = xcode_version,
        tags = ["manual"] if library != None else tags,
        testonly = testonly,
        visibility = ["//visibility:private"] if library != None else visibility,
    )
    if library == None:
        return
    _framework_cell(
        name = name + "_library",
        library = library,
        slices = slices,
        xcode_version = xcode_version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    apple_bundle_version(
        name = name + "_version",
        build_version = version,
        short_version_string = version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    if len(infoplists) == 0:
        write_file(
            name = name + "_infoplist",
            out = name + "_Info.plist",
            content = [_FRAMEWORK_INFO_PLIST],
            tags = ["manual"],
            testonly = testonly,
            visibility = ["//visibility:private"],
        )
        infoplists = [":" + name + "_infoplist"]
    apple_xcframework(
        name = name + "_bundle",
        bundle_id = bundle_id,
        bundle_name = module,
        data = resources,
        infoplists = infoplists,
        ios = _slice_dict(slices),
        minimum_os_versions = {"ios": deployment},
        public_hdrs = hdrs,
        version = ":" + name + "_version",
        deps = [":" + name + "_library"] + deps,
        tags = ["manual"],
        testonly = testonly,
        target_compatible_with = _APPLE_HOSTS,
        visibility = ["//visibility:private"],
    )
    _xcframework_check(
        name = name,
        bundle_id = bundle_id,
        bundle_name = module,
        deployment = deployment,
        hdrs = hdrs,
        resources = resources,
        selection = ":" + name + "_selection",
        slices = slices,
        version = version,
        xcframework = ":" + name + "_bundle",
        xcode_version = xcode_version,
        tags = tags,
        testonly = testonly,
        target_compatible_with = _APPLE_HOSTS,
        visibility = visibility,
    )
