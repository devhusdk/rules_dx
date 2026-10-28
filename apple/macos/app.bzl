"""macOS applications over Rust static libraries, checked and shipped ad hoc signed."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("@rules_apple//apple:macos.bzl", "macos_application")
load("@rules_apple//apple:providers.bzl", "AppleBinaryInfo", "AppleBundleInfo")
load("@rules_apple//apple:versioning.bzl", "apple_bundle_version")
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
    "label_text",
    "resolved_xcode",
)
load(
    "//apple/provisioning:defs.bzl",
    "XCODE_LINEAGE",
    "ceiling_for",
    "deployment_floor",
    "version_below",
)
load("//libs/starlark:resources.bzl", "dx_apple_compile_resources")

_CELL = "macos/arm64"

_ARCHES = ["arm64"]

_MACOS = ["@platforms//os:macos"]

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
</dict>
</plist>
"""

def default_macos_app():
    """Returns the default macOS application composition."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        frameworks = [],
        library = "",
        plugins = [],
        provisioning_profile = "",
        resources = [],
        srcs = [],
        version = "",
        xcode_version = "",
    )

def plugin_errors(plugins):
    """Returns one error string per empty or repeated plugin."""
    errors = []
    seen = []
    for plugin in plugins:
        if plugin == "":
            errors.append("apple_macos_app: plugin names must not be empty")
        elif plugin in seen:
            errors.append("apple_macos_app: plugin '" + plugin + "' is listed twice")
        seen.append(plugin)
    return errors

def _dotted(version):
    parts = version.split(".")
    for part in parts:
        if part == "" or not part.isdigit():
            return False
    return True

def _deployment_errors(deployment):
    if deployment == "":
        return ["apple_macos_app: deployment names the minimum macOS version"]
    if not _dotted(deployment):
        return ["apple_macos_app: deployment '" + deployment + "' is not a dotted version"]
    errors = []
    floor = deployment_floor("macosx")
    if version_below(deployment, floor):
        errors.append("apple_macos_app: deployment '" + deployment + "' is below " + floor)
    ceiling = ceiling_for("macosx")
    if version_below(ceiling, deployment):
        errors.append("apple_macos_app: deployment '" + deployment + "' is above the Xcode 16 'macosx' ceiling " + ceiling)
    return errors

def macos_app_errors(config):
    """Returns one error string per rejected field of one macOS app composition."""
    errors = []
    if config.arch not in _ARCHES:
        errors.append("apple_macos_app: arch '" + config.arch + "' is outside macos: want " + ", ".join(_ARCHES))
    errors.extend(_deployment_errors(config.deployment))
    if config.xcode_version != "":
        if not _dotted(config.xcode_version):
            errors.append("apple_macos_app: xcode_version '" + config.xcode_version + "' is not a dotted version")
        elif not config.xcode_version.startswith(XCODE_LINEAGE):
            errors.append("apple_macos_app: xcode '" + config.xcode_version + "' is outside " + XCODE_LINEAGE.rstrip("."))
    errors.extend(bundle_id_errors("apple_macos_app", config.bundle_id))
    errors.extend(bundle_version_errors("apple_macos_app", config.version))
    errors.extend(plugin_errors(config.plugins))
    if config.provisioning_profile != "" or config.entitlements != "":
        errors.append("apple_macos_app: building signs ad hoc only; signing is a separate operation")
    if len(config.srcs) == 0 and config.library == "":
        errors.append("apple_macos_app: srcs or library is required; pass the native entry point or a Rust static library")
    return errors

def _macos_selection_impl(ctx):
    failures = macos_app_errors(struct(
        arch = ctx.attr.arch,
        bundle_id = ctx.attr.bundle_id,
        deployment = ctx.attr.deployment,
        entitlements = ctx.attr.entitlements,
        frameworks = list(ctx.attr.framework_imports),
        library = ctx.attr.library,
        plugins = list(ctx.attr.plugins),
        provisioning_profile = ctx.attr.provisioning_profile,
        resources = list(ctx.attr.resources),
        srcs = list(ctx.attr.srcs),
        version = ctx.attr.version,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return [CcInfo()]

_macos_selection = rule(
    implementation = _macos_selection_impl,
    doc = "Fails analysis when a macOS app declaration is invalid.",
    attrs = {
        "arch": attr.string(doc = "Target CPU architecture of the app.", mandatory = True),
        "bundle_id": attr.string(doc = "Reverse DNS bundle identifier of the app.", mandatory = True),
        "deployment": attr.string(doc = "Minimum macOS version the app supports.", mandatory = True),
        "entitlements": attr.string(doc = "Entitlements label text, which building rejects.", mandatory = True),
        "framework_imports": attr.string_list(doc = "Label texts of embedded frameworks.", mandatory = True),
        "library": attr.string(doc = "Label text of the Rust library the app links.", mandatory = True),
        "plugins": attr.string_list(doc = "Label texts of plugins placed under PlugIns.", mandatory = True),
        "provisioning_profile": attr.string(doc = "Provisioning profile label text, which building rejects.", mandatory = True),
        "resources": attr.string_list(doc = "Label texts of resource files bundled into the app.", mandatory = True),
        "srcs": attr.string_list(doc = "Label texts of native sources, including the entry point.", mandatory = True),
        "version": attr.string(doc = "Bundle version string of the app.", mandatory = True),
        "xcode_version": attr.string(doc = "Xcode version the app build is pinned to.", mandatory = True),
    },
)

def _has(ctx, constraint):
    return ctx.target_platform_has_constraint(constraint[platform_common.ConstraintValueInfo])

def _target_cell(ctx):
    if not _has(ctx, ctx.attr._macos):
        return ""
    if _has(ctx, ctx.attr._arm64):
        return "macos/arm64"
    if _has(ctx, ctx.attr._x86_64):
        return "macos/x86_64"
    return "macos/unknown"

def _macos_cell_impl(ctx):
    got = _target_cell(ctx)
    if got == "":
        fail("apple_macos_app: library cell {} builds for no macOS platform here; build the app target so rules_apple selects the cell".format(_CELL))
    if got != _CELL:
        fail("apple_macos_app: app declares {} but this build targets {}; pass --macos_cpus=arm64".format(_CELL, got))
    xcode_config, xcode = resolved_xcode(ctx, "apple_macos_app", "app")
    inputs = []
    cc_infos = []
    if ctx.attr.library != None:
        cc_info = ctx.attr.library[CcInfo]
        found = archives(cc_info)
        if len(found) == 0:
            fail("apple_macos_app: library {} links no static archive; pass a Rust static library".format(display(ctx.attr.library.label)))
        inputs.extend(found)
        cc_infos.append(cc_info)
    report = check_cell(
        ctx,
        "apple_macos_app",
        _CELL,
        "macosx",
        "1",
        apple_common.platform.macos,
        apple_common.platform_type.macos,
        "AppleMacosLibraryCell",
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

_macos_cell = rule(
    implementation = _macos_cell_impl,
    doc = "Checks and forwards the app library for the macOS arm64 cell.",
    attrs = {
        "library": attr.label(
            doc = "Rust static library target linked into the app.",
            providers = [CcInfo],
        ),
        "xcode_version": attr.string(
            doc = "Xcode version the build must belong to, or empty to skip the check.",
            default = "",
        ),
        "_arm64": attr.label(default = "@platforms//cpu:arm64"),
        "_check": attr.label(
            default = Label("//apple/bundle:library_check.sh"),
            allow_single_file = True,
            cfg = "exec",
            executable = True,
        ),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
        "_xcode_config": attr.label(
            default = configuration_field(
                name = "xcode_config_label",
                fragment = "apple",
            ),
        ),
    },
    fragments = ["apple"],
)

def _runfiles_path(ctx, file):
    path = file.short_path
    if path.startswith("../"):
        return path[3:]
    return ctx.workspace_name + "/" + path

def _lines(names):
    return "\n".join(names)

def owner_relative_path(short_path, package):
    """Returns a file path relative to its owning package, as rules_apple places it."""
    path = short_path
    if path.startswith("../"):
        path = path[3:].partition("/")[2]
    if package and path.startswith(package + "/"):
        path = path[len(package) + 1:]
    return path

def _framework_names(frameworks):
    return [framework[AppleBundleInfo].bundle_name for framework in frameworks]

def _plugin_names(plugins):
    names = []
    for plugin in plugins:
        found = []
        if AppleBundleInfo in plugin:
            info = plugin[AppleBundleInfo]
            found.append(info.bundle_name + info.bundle_extension)
        elif AppleBinaryInfo in plugin:
            found.append(plugin[AppleBinaryInfo].binary.basename)
        else:
            for file in plugin.files.to_list():
                found.append(owner_relative_path(file.short_path, file.owner.package if file.owner else plugin.label.package))
        if not found:
            fail("apple_macos_app: plugin '{}' places nothing under PlugIns".format(display(plugin.label)))
        names.extend(found)
    return sorted(names)

def _macos_app_check_impl(ctx):
    bundle = ctx.attr.app[0]
    info = bundle[AppleBundleInfo]
    if info.archive.is_directory:
        fail("apple_macos_app: '{}' built a bundle directory; want a zipped .app".format(display(bundle.label)))
    xcode_config, xcode = resolved_xcode(ctx, "apple_macos_app", "app")
    identity = ctx.actions.declare_file(ctx.label.name + ".app.identity.json")
    zipped = ctx.actions.declare_file(ctx.label.name + ".zip")
    args = ctx.actions.args()
    args.add(identity)
    args.add(zipped)
    args.add(info.archive)
    args.add_all([
        info.bundle_name,
        info.bundle_extension,
        info.executable_name,
        info.bundle_id,
        ctx.attr.version,
        ctx.attr.deployment,
        ctx.attr.arch,
        xcode,
        str(xcode_config.sdk_version_for_platform(apple_common.platform.macos)),
        _lines(_framework_names(ctx.attr.frameworks)),
        _lines(_plugin_names(ctx.attr.plugins)),
        _lines(flat_resources(ctx.files.resources)),
    ])
    env = {"PATH": TOOL_PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    ctx.actions.run(
        executable = ctx.executable._check,
        arguments = [args],
        inputs = [info.archive],
        outputs = [identity, zipped],
        env = env,
        execution_requirements = xcode_config.execution_info(),
        mnemonic = "AppleMacosAppCheck",
        resource_set = dx_apple_compile_resources,
        progress_message = "Checking the macOS app bundle of %{label}",
    )
    launcher = ctx.actions.declare_file(ctx.label.name + "_run.sh")
    ctx.actions.symlink(output = launcher, target_file = ctx.file._run, is_executable = True)
    groups = {}
    validation = [identity]
    if OutputGroupInfo in bundle:
        for group in dir(bundle[OutputGroupInfo]):
            groups[group] = getattr(bundle[OutputGroupInfo], group)
        if "_validation" in groups:
            validation = depset([identity], transitive = [groups["_validation"]])
    groups["_validation"] = validation
    return [
        DefaultInfo(
            files = depset([zipped, identity]),
            executable = launcher,
            runfiles = ctx.runfiles(files = [zipped]),
        ),
        RunEnvironmentInfo(environment = {"DX_MACOS_APP_ZIP": _runfiles_path(ctx, zipped)}),
        info,
        OutputGroupInfo(**groups),
    ]

_macos_app_check = rule(
    implementation = _macos_app_check_impl,
    doc = "Checks one upstream macOS application bundle and forwards it with its identity.",
    attrs = {
        "app": attr.label(
            doc = "The upstream macos_application, built ad hoc signed.",
            cfg = adhoc_signing_transition,
            mandatory = True,
            providers = [AppleBundleInfo],
        ),
        "arch": attr.string(doc = "Architecture every Mach-O file of the app must carry.", mandatory = True),
        "deployment": attr.string(doc = "Minimum macOS version the app supports.", mandatory = True),
        "frameworks": attr.label_list(
            doc = "Frameworks the bundle must embed under Frameworks.",
            providers = [AppleBundleInfo],
        ),
        "plugins": attr.label_list(doc = "Plugins the bundle must hold under PlugIns.", allow_files = True),
        "resources": attr.label_list(doc = "Resource files the bundle must hold.", allow_files = True),
        "version": attr.string(doc = "Bundle version string of the app.", mandatory = True),
        "xcode_version": attr.string(doc = "Xcode version the app build is pinned to.", default = ""),
        "_check": attr.label(
            default = ":app_check.sh",
            allow_single_file = True,
            cfg = "exec",
            executable = True,
        ),
        "_run": attr.label(default = ":run_app.sh", allow_single_file = True),
        "_xcode_config": attr.label(
            default = configuration_field(
                name = "xcode_config_label",
                fragment = "apple",
            ),
        ),
    },
    executable = True,
    fragments = ["apple"],
)

def apple_macos_app(
        name,
        srcs = [],
        hdrs = [],
        deps = [],
        library = None,
        frameworks = [],
        plugins = [],
        resources = [],
        infoplists = [],
        bundle_id = "",
        version = "",
        deployment = "",
        arch = "arm64",
        xcode_version = "",
        entitlements = None,
        provisioning_profile = None,
        tags = [],
        testonly = False,
        visibility = None):
    """Builds one upstream macOS application over a Rust library, checked and ad hoc signed."""
    _macos_selection(
        name = name + "_selection",
        arch = arch,
        bundle_id = bundle_id,
        deployment = deployment,
        entitlements = label_text(entitlements),
        framework_imports = [label_text(framework) for framework in frameworks],
        library = label_text(library),
        plugins = [label_text(plugin) for plugin in plugins],
        provisioning_profile = label_text(provisioning_profile),
        resources = [label_text(resource) for resource in resources],
        srcs = [label_text(src) for src in srcs],
        version = version,
        xcode_version = xcode_version,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    entry_deps = [":" + name + "_selection"]
    if library != None:
        _macos_cell(
            name = name + "_library",
            library = library,
            xcode_version = xcode_version,
            tags = ["manual"],
            testonly = testonly,
            target_compatible_with = _MACOS,
            visibility = ["//visibility:private"],
        )
        entry_deps.append(":" + name + "_library")
    objc_library(
        name = name + "_entry",
        srcs = srcs,
        hdrs = hdrs,
        deps = entry_deps + frameworks + deps,
        tags = ["manual"],
        testonly = testonly,
        target_compatible_with = _MACOS,
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
    macos_application(
        name = name + "_bundle",
        additional_contents = {plugin: "PlugIns" for plugin in plugins},
        bundle_id = bundle_id,
        bundle_name = name,
        frameworks = frameworks,
        infoplists = infoplists,
        minimum_os_version = deployment,
        resources = resources,
        version = ":" + name + "_version",
        deps = [":" + name + "_entry"],
        tags = ["manual"],
        testonly = testonly,
        target_compatible_with = _MACOS,
        visibility = ["//visibility:private"],
    )
    _macos_app_check(
        name = name,
        app = ":" + name + "_bundle",
        arch = arch,
        deployment = deployment,
        frameworks = frameworks,
        plugins = plugins,
        resources = resources,
        version = version,
        xcode_version = xcode_version,
        tags = tags,
        testonly = testonly,
        target_compatible_with = _MACOS,
        visibility = visibility,
    )
