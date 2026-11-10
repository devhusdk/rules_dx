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
load("@rules_dx//libs/starlark:resources.bzl", "dx_heavy_resources")
load("//bundle:defs.bzl", "SIGNING_CERTIFICATE", "TOOL_PATH", "archives", "check_cell", "display", "flat_resources", "framework_binaries", "label_text", "resolved_xcode")
load(
    ":cell.bzl",
    "APPLE_HOSTS",
    "CELL_ATTRS",
    "MACH_O_PLATFORMS",
    "MULTI_CPUS",
    "ios_platform",
    "target_cell",
)
load(":defs.bzl", "AppleIosLibInfo", "ios_platform_errors", "ios_slice", "split_platform")
load(":app_validate.bzl", "ios_app_errors", "ios_test_coverage_kwargs", "ios_test_errors", "ios_test_source_errors", "ios_xcframework_errors", "ios_xcframework_header_name_errors")
load(":validate.bzl", "ios_arch_errors", "ios_environment_errors", "ios_sdk_for")

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

def _app_selection_impl(ctx):
    selected = ios_platform_errors("apple_ios_app", ctx.attr.environment)
    if len(selected) > 0:
        fail("; ".join(selected))
    failures = ios_app_errors(struct(
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
    errors = ios_environment_errors(ctx.attr.environment) + ios_arch_errors(ctx.attr.environment, ctx.attr.arch)
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
        ios_sdk_for(ctx.attr.environment),
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
        fail("apple_ios_xcframework: framework slices build for no iOS platform here; build the framework target so rules_apple selects each slice")
    if got not in ctx.attr.slices:
        fail("apple_ios_xcframework: this build targets {}, outside the framework slices {}".format(got, ", ".join(ctx.attr.slices)))
    library = ctx.attr.library
    if AppleIosLibInfo in library and library[AppleIosLibInfo].cell != got:
        fail("apple_ios_xcframework: library {} declares {} but this slice builds {}".format(
            display(library.label),
            library[AppleIosLibInfo].cell,
            got,
        ))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_xcframework", "framework")
    cc_info = ctx.attr.library[CcInfo]
    found = archives(cc_info)
    if len(found) == 0:
        fail("apple_ios_xcframework: library {} links no static archive; pass a Rust static library".format(display(ctx.attr.library.label)))
    environment = got.split("/")[0]
    report = check_cell(
        ctx,
        "apple_ios_xcframework",
        got,
        ios_sdk_for(environment),
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

_IOS_MULTI_CPUS = "//command_line_option:ios_multi_cpus"

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

def _device_cpu_impl(_settings, _attr):
    return {
        _IOS_MULTI_CPUS: [MULTI_CPUS["device/arm64"]],
        SIGNING_CERTIFICATE: "-",
    }

_device_cpu_transition = transition(
    implementation = _device_cpu_impl,
    inputs = [],
    outputs = [_IOS_MULTI_CPUS, SIGNING_CERTIFICATE],
)

_device_app = rule(
    implementation = _device_app_impl,
    doc = "Builds one device app bundle signed ad hoc whatever signing flags are set.",
    attrs = {
        "bundle": attr.label(
            doc = "The upstream ios_application.",
            cfg = _device_cpu_transition,
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
        srcs = None,
        hdrs = [],
        deps = None,
        platform = None,
        bundle_id = "",
        version = "",
        deployment = "",
        xcode_version = "",
        resources = [],
        infoplists = [],
        families = ["iphone"],
        entitlements = None,
        provisioning_profile = None,
        tags = None,
        testonly = False,
        visibility = None):
    """Builds one upstream iOS application bundle over a Rust library cell, XCFramework imports, or a Swift @main module."""
    srcs = srcs or []
    deps = deps or []
    tags = tags or []
    composed = library != None or len(frameworks) > 0 or entry != None
    environment, arch = split_platform(ios_slice(platform))
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
    failures = ios_test_errors(ctx.attr.device_type, ctx.attr.os_version, ctx.attr.deployment)
    failures.extend(ios_test_source_errors(ctx.attr.srcs, ctx.attr.dep_labels))
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
        "dep_labels": attr.string_list(
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
        srcs = None,
        deployment = "",
        device_type = "",
        os_version = "",
        deps = None,
        tags = None,
        visibility = None,
        **kwargs):
    """Runs XCTest cases inside one app on a declared simulator device and runtime."""
    srcs = srcs or []
    deps = deps or []
    tags = tags or []
    _test_selection(
        name = name + "_selection",
        deployment = deployment,
        device_type = device_type,
        os_version = os_version,
        srcs = [label_text(src) for src in srcs],
        dep_labels = [label_text(dep) for dep in deps],
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
        clean_up_simulator_action = Label("//ios:delete_test_simulator"),
        create_simulator_action = Label("//ios:create_test_simulator"),
        device_type = device_type,
        export_covered_binaries = True,
        os_version = os_version,
        post_action = Label("@rules_dx//tools/coverage/collect:apple_post_action"),
        post_action_determines_exit_code = True,
        reuse_simulator = False,
        tags = ["manual"],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    env = kwargs.pop("env", {})
    kwargs.update(ios_test_coverage_kwargs(env))
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

def _framework_selection_impl(ctx):
    failures = ios_xcframework_errors(struct(
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
        fail("apple_ios_xcframework: {} built {} .xcframework.zip outputs, want exactly one".format(
            display(ctx.attr.xcframework.label),
            len(archives),
        ))
    xcode_config, xcode = resolved_xcode(ctx, "apple_ios_xcframework", "framework")
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
    header_failures = ios_xcframework_header_name_errors(header_names, ctx.attr.bundle_name)
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
        resource_set = dx_heavy_resources,
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

def apple_ios_xcframework(
        name,
        library = None,
        hdrs = [],
        deps = None,
        bundle_id = "",
        bundle_name = "",
        version = "",
        deployment = "",
        xcode_version = "",
        platforms = [],
        resources = [],
        infoplists = [],
        tags = None,
        testonly = False,
        visibility = None):
    """Builds and checks one XCFramework over a Rust static library and public headers."""
    deps = deps or []
    tags = tags or []
    module = bundle_name or name
    _framework_selection(
        name = name + "_selection" if library != None else name,
        bundle_id = bundle_id,
        bundle_name = module,
        deployment = deployment,
        hdrs = [label_text(header) for header in hdrs],
        library = label_text(library),
        resources = [label_text(resource) for resource in resources],
        slices = platforms,
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
        slices = platforms,
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
        ios = _slice_dict(platforms),
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
        slices = platforms,
        version = version,
        xcframework = ":" + name + "_bundle",
        xcode_version = xcode_version,
        tags = tags,
        testonly = testonly,
        target_compatible_with = APPLE_HOSTS,
        visibility = visibility,
    )
