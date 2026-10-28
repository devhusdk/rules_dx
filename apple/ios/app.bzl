"""Simulator and device applications and XCFrameworks over Rust static libraries."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("@rules_apple//apple:apple.bzl", "apple_xcframework")
load("@rules_apple//apple:ios.bzl", "ios_application", "ios_unit_test")
load("@rules_apple//apple:providers.bzl", "AppleBundleInfo")
load("@rules_apple//apple:versioning.bzl", "apple_bundle_version")
load("@rules_apple//apple/testing/default_runner:ios_xctestrun_runner.bzl", "ios_xctestrun_runner")
load("@rules_cc//cc:objc_library.bzl", "objc_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(
    "//apple/bundle:defs.bzl",
    "TOOL_PATH",
    "adhoc_signing_transition",
    "archives",
    "bundle_id_errors",
    "bundle_version_errors",
    "check_cell",
    "display",
    "flat_resources",
    "framework_binaries",
    "label_text",
    "resolved_xcode",
)
load("//apple/provisioning:defs.bzl", "version_below")
load("//libs/starlark:resources.bzl", "dx_apple_compile_resources")
load(
    ":cell.bzl",
    "APPLE_HOSTS",
    "CELL_ATTRS",
    "MACH_O_PLATFORMS",
    "MULTI_CPUS",
    "ios_platform",
    "target_cell",
)
load(
    ":defs.bzl",
    "AppleIosLibInfo",
    "arch_errors",
    "arches",
    "deployment_errors",
    "environment_errors",
    "environments",
    "sdk_for",
    "xcode_errors",
)

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
            return ["apple_app: device builds need provisioning_profile, a .mobileprovision file"]
        if not provisioning_profile.endswith(".mobileprovision"):
            return ["apple_app: provisioning_profile '" + provisioning_profile + "' must be a .mobileprovision file"]
        return []
    if provisioning_profile != "":
        return ["apple_app: simulator builds sign ad hoc; leave provisioning_profile empty"]
    return []

def entry_errors(srcs, entry):
    """Returns one error string unless exactly one of srcs or entry names the app entry point."""
    if len(srcs) == 0 and entry == "":
        return ["apple_app: srcs needs the native entry point that calls UIApplicationMain, or entry names a Swift @main module"]
    if len(srcs) > 0 and entry != "":
        return ["apple_app: set srcs or entry for the app entry point, not both"]
    return []

def test_source_errors(srcs, deps):
    """Returns one error string when a test has neither native sources nor test modules."""
    if len(srcs) == 0 and len(deps) == 0:
        return ["apple_app_test: srcs or deps needs the XCTest cases"]
    return []

def default_app(environment):
    """Returns the default simulator application composition for one environment."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        entry = "",
        environment = environment,
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
    errors.extend(entry_errors(config.srcs, config.entry))
    if config.library == "" and len(config.frameworks) == 0 and config.entry == "":
        errors.append("apple_app: app composes one qualified Rust library cell in library or XCFramework imports in frameworks, or a Swift @main module in entry")
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
        entry = ctx.attr.entry,
        environment = ctx.attr.environment,
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
    doc = "Fails analysis when an iOS app declaration is invalid.",
    attrs = {
        "arch": attr.string(
            doc = "Target CPU architecture built for the selected environment.",
            mandatory = True,
        ),
        "bundle_id": attr.string(
            doc = "Reverse DNS bundle identifier of the app.",
            mandatory = True,
        ),
        "deployment": attr.string(doc = "Minimum iOS version the app supports.", mandatory = True),
        "entitlements": attr.string(
            doc = "Path of the entitlements file, or empty for none.",
            mandatory = True,
        ),
        "entry": attr.string(
            doc = "Label text of the Swift @main module, or empty.",
            mandatory = True,
        ),
        "environment": attr.string(
            doc = "Target environment, such as device or simulator.",
            mandatory = True,
        ),
        "framework_imports": attr.string_list(
            doc = "Label texts of XCFramework imports linked into the app.",
            mandatory = True,
        ),
        "library": attr.string(
            doc = "Label text of the Rust library cell the app links.",
            mandatory = True,
        ),
        "provisioning_profile": attr.string(
            doc = "Path of the provisioning profile used for device builds.",
            mandatory = True,
        ),
        "resources": attr.string_list(
            doc = "Label texts of resource files bundled into the app.",
            mandatory = True,
        ),
        "srcs": attr.string_list(
            doc = "Label texts of native sources, including the UIApplicationMain entry point.",
            mandatory = True,
        ),
        "version": attr.string(doc = "Bundle version string of the app.", mandatory = True),
        "xcode_version": attr.string(
            doc = "Xcode version the app build is pinned to.",
            mandatory = True,
        ),
    },
)

def _library_cell_impl(ctx):
    errors = environment_errors(ctx.attr.environment) + arch_errors(ctx.attr.environment, ctx.attr.arch)
    if len(errors) > 0:
        fail("; ".join(errors))
    want = ctx.attr.environment + "/" + ctx.attr.arch
    got = target_cell(ctx)
    if got == "":
        fail("apple_ios_app: library cell {} builds for no iOS platform here; build the app target so rules_apple selects the cell".format(want))
    if got != want:
        fail("apple_ios_app: app declares {} but this build targets {}; pass --ios_multi_cpus={}".format(
            want,
            got,
            MULTI_CPUS[want],
        ))
    library = ctx.attr.library
    if library != None and AppleIosLibInfo in library and library[AppleIosLibInfo].cell != want:
        fail("apple_ios_app: library {} declares {} but the app builds {}".format(
            display(library.label),
            library[AppleIosLibInfo].cell,
            want,
        ))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_app", "app")
    inputs = []
    cc_infos = []
    if ctx.attr.library != None:
        cc_info = ctx.attr.library[CcInfo]
        found = archives(cc_info)
        if len(found) == 0:
            fail("apple_ios_app: library {} links no static archive; pass a Rust static library".format(display(ctx.attr.library.label)))
        inputs.extend(found)
        cc_infos.append(cc_info)
    for framework in ctx.attr.frameworks:
        binaries = framework_binaries(framework[CcInfo])
        if len(binaries) == 0:
            fail("apple_ios_app: framework {} links no framework binary; pass an apple_dynamic_xcframework_import".format(display(framework.label)))
        inputs.extend(binaries)
    report = check_cell(
        ctx,
        "apple_ios_app",
        want,
        sdk_for(ctx.attr.environment),
        MACH_O_PLATFORMS[ctx.attr.environment],
        ios_platform(ctx.attr.environment),
        apple_common.platform_type.ios,
        "AppleIosLibraryCell",
        xcode_config,
        xcode,
        inputs,
    )
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

_library_cell = rule(
    implementation = _library_cell_impl,
    doc = "Checks and forwards the app library for one iOS environment and arch.",
    attrs = dict(CELL_ATTRS, **{
        "arch": attr.string(
            doc = "Target CPU architecture the library cell is built for.",
            mandatory = True,
        ),
        "environment": attr.string(
            doc = "Target environment the library cell is built for.",
            mandatory = True,
        ),
        "frameworks": attr.label_list(
            doc = "XCFramework import targets whose binaries are linked into the app.",
            providers = [CcInfo],
        ),
        "library": attr.label(
            doc = "Rust static library target linked into the app.",
            providers = [CcInfo],
        ),
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
    got = target_cell(ctx)
    if got == "":
        fail("apple_ios_framework: framework slices build for no iOS platform here; build the framework target so rules_apple selects each slice")
    if got not in ctx.attr.slices:
        fail("apple_ios_framework: this build targets {}, outside the framework slices {}".format(got, ", ".join(ctx.attr.slices)))
    library = ctx.attr.library
    if AppleIosLibInfo in library and library[AppleIosLibInfo].cell != got:
        fail("apple_ios_framework: library {} declares {} but this slice builds {}".format(
            display(library.label),
            library[AppleIosLibInfo].cell,
            got,
        ))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_framework", "framework")
    cc_info = ctx.attr.library[CcInfo]
    found = archives(cc_info)
    if len(found) == 0:
        fail("apple_ios_framework: library {} links no static archive; pass a Rust static library".format(display(ctx.attr.library.label)))
    environment = got.split("/")[0]
    report = check_cell(
        ctx,
        "apple_ios_framework",
        got,
        sdk_for(environment),
        MACH_O_PLATFORMS[environment],
        ios_platform(environment),
        apple_common.platform_type.ios,
        "AppleIosLibraryCell",
        xcode_config,
        xcode,
        found,
    )
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
    doc = "Checks and forwards the framework library for one iOS slice.",
    attrs = dict(CELL_ATTRS, **{
        "library": attr.label(
            doc = "Rust static library target checked and forwarded for each slice.",
            mandatory = True,
            providers = [CcInfo],
        ),
        "slices": attr.string_list(
            doc = "Environment and arch slices to check, such as device/arm64.",
            mandatory = True,
        ),
    }),
    fragments = ["apple"],
)

AppleIosDeviceAppInfo = provider(
    doc = "One ad hoc signed device app ready for an explicit export.",
    fields = {
        "bundle_id": "The app bundle identifier.",
        "ipa": "The ad hoc signed .ipa File.",
        "profile": "The declared .mobileprovision File.",
    },
)

def _device_app_impl(ctx):
    bundle = ctx.attr.bundle[0] if type(ctx.attr.bundle) == "list" else ctx.attr.bundle
    ipas = [file for file in bundle[DefaultInfo].files.to_list() if file.extension == "ipa"]
    if len(ipas) != 1:
        fail("apple_ios_app: '{}' built {} .ipa outputs, want exactly one".format(display(bundle.label), len(ipas)))
    ipa = ctx.actions.declare_file(ctx.label.name + ".ipa")
    ctx.actions.symlink(output = ipa, target_file = ipas[0])
    providers = [
        DefaultInfo(
            files = depset([ipa]),
            runfiles = ctx.runfiles(files = [ipa]).merge(bundle[DefaultInfo].default_runfiles),
        ),
        AppleIosDeviceAppInfo(
            bundle_id = bundle[AppleBundleInfo].bundle_id,
            ipa = ipa,
            profile = ctx.file.profile,
        ),
        bundle[AppleBundleInfo],
    ]
    if OutputGroupInfo in bundle:
        providers.append(bundle[OutputGroupInfo])
    return providers

_device_app = rule(
    implementation = _device_app_impl,
    doc = "Builds one device app bundle signed ad hoc whatever signing flags are set.",
    attrs = {
        "bundle": attr.label(
            doc = "The upstream ios_application.",
            cfg = adhoc_signing_transition,
            mandatory = True,
            providers = [AppleBundleInfo],
        ),
        "profile": attr.label(
            doc = "The declared .mobileprovision file.",
            allow_single_file = [".mobileprovision"],
        ),
    },
)

def apple_ios_app(
        name,
        library = None,
        frameworks = [],
        entry = None,
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
        tags = [],
        testonly = False,
        visibility = None):
    """Builds one upstream iOS application bundle over a Rust library cell, XCFramework imports, or a Swift @main module.

    entry takes any CcInfo or SwiftInfo target holding the @main module, such as a rules_swift swift_library, instead of srcs.
    """
    composed = library != None or len(frameworks) > 0 or entry != None
    _app_selection(
        name = name + "_selection" if composed else name,
        arch = arch,
        bundle_id = bundle_id,
        deployment = deployment,
        entitlements = label_text(entitlements),
        entry = label_text(entry),
        environment = environment,
        framework_imports = [label_text(framework) for framework in frameworks],
        library = label_text(library),
        provisioning_profile = label_text(provisioning_profile),
        resources = [label_text(resource) for resource in resources],
        srcs = [label_text(src) for src in srcs],
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
    if entry == None:
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
            target_compatible_with = APPLE_HOSTS,
            visibility = ["//visibility:private"],
        )
        app_deps = [":" + name + "_entry"]
    else:
        app_deps = [
            entry,
            ":" + name + "_selection",
            ":" + name + "_library",
        ] + frameworks + deps
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
    if environment == "device":
        ios_application(
            name = name + "_bundle",
            bundle_id = bundle_id,
            bundle_name = name,
            entitlements = entitlements,
            entitlements_validation = "error",
            families = families,
            infoplists = infoplists,
            minimum_os_version = deployment,
            provisioning_profile = provisioning_profile,
            resources = resources,
            version = ":" + name + "_version",
            deps = app_deps,
            tags = ["manual"],
            testonly = testonly,
            target_compatible_with = APPLE_HOSTS,
            visibility = ["//visibility:private"],
        )
        _device_app(
            name = name,
            bundle = ":" + name + "_bundle",
            profile = provisioning_profile,
            tags = tags,
            testonly = testonly,
            target_compatible_with = APPLE_HOSTS,
            visibility = visibility,
        )
        return
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
        deps = app_deps,
        tags = tags,
        testonly = testonly,
        target_compatible_with = APPLE_HOSTS,
        visibility = visibility,
    )

def _test_selection_impl(ctx):
    failures = test_errors(ctx.attr.device_type, ctx.attr.os_version, ctx.attr.deployment)
    failures.extend(test_source_errors(ctx.attr.srcs, ctx.attr.deps))
    if len(failures) > 0:
        fail("; ".join(failures))
    return [CcInfo()]

_test_selection = rule(
    implementation = _test_selection_impl,
    doc = "Fails analysis when an iOS app test declaration is invalid.",
    attrs = {
        "deployment": attr.string(
            doc = "Minimum iOS version of the host app under test.",
            mandatory = True,
        ),
        "device_type": attr.string(
            doc = "Simulator device type to run tests on, such as iPhone 16.",
            mandatory = True,
        ),
        "deps": attr.string_list(
            doc = "Label texts of test modules.",
            mandatory = True,
        ),
        "os_version": attr.string(
            doc = "Simulator runtime version to run tests on, such as 18.5.",
            mandatory = True,
        ),
        "srcs": attr.string_list(
            doc = "Label texts of native XCTest sources.",
            mandatory = True,
        ),
    },
)

def apple_ios_app_test(
        name,
        app,
        srcs = [],
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
        srcs = [label_text(src) for src in srcs],
        deps = [label_text(dep) for dep in deps],
        tags = ["manual"],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    if len(srcs) > 0:
        objc_library(
            name = name + "_lib",
            srcs = srcs,
            deps = [":" + name + "_selection"] + deps,
            tags = ["manual"],
            testonly = True,
            target_compatible_with = APPLE_HOSTS,
            visibility = ["//visibility:private"],
        )
        test_deps = [":" + name + "_lib"]
    else:
        test_deps = [":" + name + "_selection"] + deps
    ios_xctestrun_runner(
        name = name + "_runner",
        clean_up_simulator_action = Label("//apple/ios:delete_test_simulator"),
        create_simulator_action = Label("//apple/ios:create_test_simulator"),
        device_type = device_type,
        os_version = os_version,
        reuse_simulator = False,
        tags = ["manual"],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    ios_unit_test(
        name = name,
        minimum_os_version = deployment,
        runner = ":" + name + "_runner",
        test_host = app,
        deps = test_deps,
        tags = sorted(set(tags + ["no-sandbox"])),
        target_compatible_with = APPLE_HOSTS,
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

def framework_header_errors(hdrs):
    """Returns one error string when no public header is given."""
    if len(hdrs) == 0:
        return ["apple_framework: hdrs needs at least one public header for the framework module map"]
    return []

def framework_header_name_errors(names, module):
    """Returns one error string per non-header or umbrella-clashing public header file."""
    errors = []
    umbrella = module + ".h"
    for name in names:
        if not name.endswith(".h"):
            errors.append("apple_framework: public header '" + name + "' must be a .h file")
        elif name.lower() == umbrella.lower():
            errors.append("apple_framework: public header '" + name + "' clashes with the umbrella header " + umbrella)
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
    errors.extend(framework_header_errors(config.hdrs))
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
    doc = "Fails analysis when an iOS framework declaration is invalid.",
    attrs = {
        "bundle_id": attr.string(
            doc = "Reverse DNS bundle identifier of the framework.",
            mandatory = True,
        ),
        "bundle_name": attr.string(
            doc = "Module name of the framework, defaulting to the target name.",
            mandatory = True,
        ),
        "deployment": attr.string(
            doc = "Minimum iOS version the framework supports.",
            mandatory = True,
        ),
        "hdrs": attr.string_list(
            doc = "Public header file names listed in the framework module map.",
            mandatory = True,
        ),
        "library": attr.string(
            doc = "Label text of the Rust static library the framework wraps.",
            mandatory = True,
        ),
        "resources": attr.string_list(
            doc = "Label texts of resource files bundled into the framework.",
            mandatory = True,
        ),
        "slices": attr.string_list(
            doc = "Environment and arch slices the framework is built for.",
            mandatory = True,
        ),
        "version": attr.string(doc = "Bundle version string of the framework.", mandatory = True),
        "xcode_version": attr.string(
            doc = "Xcode version the framework build is pinned to.",
            mandatory = True,
        ),
    },
)

def _xcframework_check_impl(ctx):
    archives = [file for file in ctx.attr.xcframework[DefaultInfo].files.to_list() if file.basename.endswith(".xcframework.zip")]
    if len(archives) != 1:
        fail("apple_ios_framework: {} built {} .xcframework.zip outputs, want exactly one".format(
            display(ctx.attr.xcframework.label),
            len(archives),
        ))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_framework", "framework")
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
        "\n".join(flat_resources(ctx.files.resources)),
    ])
    header_names = sorted([header.basename for header in ctx.files.hdrs])
    header_failures = framework_header_name_errors(header_names, ctx.attr.bundle_name)
    if len(header_failures) > 0:
        fail("; ".join(header_failures))
    args.add_all(header_names)
    env = {"PATH": TOOL_PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    ctx.actions.run(
        executable = ctx.executable._check,
        arguments = [args],
        inputs = archives,
        outputs = [report, out],
        env = env,
        execution_requirements = xcode_config.execution_info(),
        mnemonic = "AppleXcframeworkCheck",
        resource_set = dx_apple_compile_resources,
        progress_message = "Checking the XCFramework slices of %{label}",
    )
    return [DefaultInfo(files = depset([out, report]))]

_xcframework_check = rule(
    implementation = _xcframework_check_impl,
    doc = "Checks the slices of one XCFramework and repacks it.",
    attrs = {
        "bundle_id": attr.string(
            doc = "Reverse DNS bundle identifier of the XCFramework.",
            mandatory = True,
        ),
        "bundle_name": attr.string(
            doc = "Module name of the framework, defaulting to the target name.",
            mandatory = True,
        ),
        "deployment": attr.string(
            doc = "Minimum iOS version the XCFramework supports.",
            mandatory = True,
        ),
        "hdrs": attr.label_list(
            doc = "Public header files checked against the framework module map.",
            allow_files = [".h"],
        ),
        "resources": attr.label_list(
            doc = "Resource files bundled into the XCFramework.",
            allow_files = True,
        ),
        "selection": attr.label(
            doc = "Selection target whose validation runs before the slice check.",
            mandatory = True,
        ),
        "slices": attr.string_list(
            doc = "Environment and arch slices the XCFramework must contain.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Bundle version string of the XCFramework.",
            mandatory = True,
        ),
        "xcframework": attr.label(
            doc = "Packaged XCFramework target whose slices are checked and repacked.",
            mandatory = True,
        ),
        "xcode_version": attr.string(
            doc = "Xcode version the XCFramework build is pinned to.",
            default = "",
        ),
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
        hdrs = [label_text(header) for header in hdrs],
        library = label_text(library),
        resources = [label_text(resource) for resource in resources],
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
        target_compatible_with = APPLE_HOSTS,
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
        target_compatible_with = APPLE_HOSTS,
        visibility = visibility,
    )
