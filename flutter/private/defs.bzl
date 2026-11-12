"""Thin Flutter wrappers over rules_flutter."""

load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("@rules_dart//dart:utils.bzl", "dart_analyzable_info_with_package", "derive_lib_root")
load("@dx_rules_flutter_pub//flutter:android.bzl", _flutter_android_app = "flutter_android_app")
load("@rules_flutter//flutter:defs.bzl", _flutter_application = "flutter_application", _flutter_gen_l10n = "flutter_gen_l10n", _flutter_library = "flutter_library", _flutter_test = "flutter_test")
load("@rules_flutter//flutter:ios.bzl", _flutter_ios_app = "flutter_ios_app")
load("@rules_flutter//flutter:providers.bzl", "FlutterApplicationInfo", "FlutterInfo")
load("@rules_flutter//flutter:web.bzl", _flutter_web_app = "flutter_web_app", _flutter_web_bundle = "flutter_web_bundle")
load("@rules_apple//apple:providers.bzl", "AppleBundleInfo")
load("//apple/private/bundle:defs.bzl", "EXTRA_TOOLCHAINS_SETTING", "apple_extra_toolchains")
load("//apple/private/ios:cell.bzl", "APPLE_BUILD_HOST")
load("//dart/private:defs.bzl", "DART_QUALITY_TOOLS", "DART_SOURCE_SPECS")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:platform_support.bzl", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//flutter/private:platforms.bzl", FLUTTER_SUPPORT = "PLATFORM_SUPPORT")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

flutter_gen_l10n = _flutter_gen_l10n

FLUTTER_SOURCE_EXTS = [".dart"]

FLUTTER_COMPATIBLE_WITH = compatible_with(FLUTTER_SUPPORT)

FLUTTER_ANDROID_ABIS = ["arm64", "x64"]

_FLUTTER = dx_language_spec(what = "flutter_*", quality_specs = DART_SOURCE_SPECS, allow_files = FLUTTER_SOURCE_EXTS, quality_tools = DART_QUALITY_TOOLS)

_flutter_library_forward = dx_library_forward_rule(
    _FLUTTER,
    provides = [DartAnalyzableInfo, DartInfo, DefaultInfo, FlutterInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartInfo, "DartInfo"), (DartAnalyzableInfo, "DartAnalyzableInfo"), (FlutterInfo, "FlutterInfo")],
    upstream_providers = [[DartInfo, DartAnalyzableInfo, FlutterInfo]],
    runtime = "besteffort",
)

_flutter_forward_test = dx_executable_forward_rule(
    _FLUTTER,
    kind = "test",
    provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    upstream_providers = [[DartAnalyzableInfo]],
    extra_attrs = dx_lcov_merger_attr(),
)

_flutter_web_forward = dx_library_forward_rule(
    _FLUTTER,
    provides = [DartAnalyzableInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    upstream_providers = [[DartAnalyzableInfo]],
    runtime = "besteffort",
)

def _application_dart_info(ctx, _upstream):
    root = derive_lib_root(ctx.label.workspace_root, ctx.label.package)
    lib = (root + "/lib/") if root else "lib/"
    package_srcs = [f for f in ctx.files.srcs if f.short_path.startswith(lib)]
    entry = [f for f in ctx.files.srcs if f not in package_srcs]
    return [dart_analyzable_info_with_package(
        label = ctx.label,
        package_name = ctx.attr.package_name,
        lib_root = root,
        deps = ctx.attr.deps,
        srcs = entry,
        package_srcs = package_srcs,
        language_version = ctx.attr.language_version,
    )]

_flutter_application_forward = dx_library_forward_rule(
    _FLUTTER,
    provides = [DartAnalyzableInfo, DefaultInfo, FlutterApplicationInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(FlutterApplicationInfo, "FlutterApplicationInfo")],
    upstream_providers = [[FlutterApplicationInfo]],
    extra_attrs = {
        "deps": attr.label_list(doc = "Dart libraries the application imports.", providers = [DartInfo]),
        "language_version": attr.string(doc = "Dart language version of the application."),
        "package_name": attr.string(doc = "Dart package name of the application.", mandatory = True),
    },
    extra_providers = _application_dart_info,
    runtime = "besteffort",
)

def _invalid_impl(ctx):
    fail(ctx.attr.message)

_invalid = rule(
    implementation = _invalid_impl,
    doc = "Fails analysis with the given message.",
    attrs = {"message": attr.string(doc = "Failure message.", mandatory = True)},
)

def _declare_invalid(name, message, kwargs, visibility):
    checked = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    _invalid(
        name = name,
        message = message,
        tags = checked.get("tags", []),
        testonly = checked.get("testonly"),
        target_compatible_with = checked["target_compatible_with"],
        visibility = visibility,
    )

def _missing_font_asset(font_files, fonts_json):
    if type(fonts_json) != "string":
        return None
    provided = (font_files or {}).values()
    for family in json.decode(fonts_json):
        for font in family.get("fonts", []):
            if font.get("asset") not in provided:
                return font.get("asset")
    return None

def flutter_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_library; fonts_json assets must come from font_files."""
    missing = _missing_font_asset(kwargs.get("font_files"), kwargs.get("fonts_json"))
    if missing:
        _declare_invalid(name, "flutter_library: fonts_json names font asset '{}' but font_files does not provide it".format(missing), kwargs, visibility)
        return
    dx_wrap(name, _flutter_library, _flutter_library_forward, srcs or [], visibility = visibility, **dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH))

def flutter_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_test. main is name.dart, then test/name.dart, else the single src."""
    main = dx_infer_main("flutter_test", name, main, srcs, dx_main_candidates(name, ".dart", ["test"]))
    kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    dx_wrap_test(name, _flutter_test, _flutter_forward_test, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def flutter_application(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_application. main is name.dart, then lib/main.dart, else the single src."""
    main = dx_infer_main("flutter_application", name, main, srcs, dx_main_candidates(name, ".dart") + ["lib/main.dart"])
    kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    dx_wrap(
        name,
        _flutter_application,
        _flutter_application_forward,
        srcs,
        visibility = visibility,
        upstream_kwargs = dict(kwargs, main = main),
        quality_srcs = dx_entry_quality_srcs(main, srcs),
        extra_forward_kwargs = {
            "deps": kwargs.get("deps", []),
            "language_version": kwargs.get("language_version", ""),
            "package_name": kwargs["package_name"],
        },
        **kwargs
    )

def flutter_web_app(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_web_app. main is name.dart, then lib/main.dart, else the single src."""
    main = dx_infer_main("flutter_web_app", name, main, srcs, dx_main_candidates(name, ".dart") + ["lib/main.dart"])
    kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    dx_wrap_binary(name, _flutter_web_app, _flutter_web_forward, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def flutter_web_bundle(name, **kwargs):
    """Thin wrapper over flutter_web_bundle, for dart2js or canvaskit builds."""
    _flutter_web_bundle(name = name, **dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH))

def flutter_android_app(name, application, package_name, android_abi = "arm64", visibility = None, **kwargs):
    """Thin wrapper over flutter_android_app. An unknown android_abi fails when the target is analyzed."""
    if android_abi not in FLUTTER_ANDROID_ABIS:
        _declare_invalid(name, "flutter_android_app: android_abi '{}' is unknown: want {}".format(android_abi, ", ".join(FLUTTER_ANDROID_ABIS)), kwargs, visibility)
        return
    app_kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    _flutter_android_app(
        name = name,
        application = application,
        package_name = package_name,
        android_abi = android_abi,
        visibility = visibility,
        **app_kwargs
    )

_IOS_MULTI_CPUS = "//command_line_option:ios_multi_cpus"

_COMPILATION_MODE = "//command_line_option:compilation_mode"

def _ios_simulator_impl(settings, _attr):
    return {
        EXTRA_TOOLCHAINS_SETTING: apple_extra_toolchains(settings[EXTRA_TOOLCHAINS_SETTING]),
        _COMPILATION_MODE: "dbg",
        _IOS_MULTI_CPUS: ["sim_arm64"],
    }

_ios_simulator_transition = transition(
    implementation = _ios_simulator_impl,
    inputs = [EXTRA_TOOLCHAINS_SETTING],
    outputs = [EXTRA_TOOLCHAINS_SETTING, _COMPILATION_MODE, _IOS_MULTI_CPUS],
)

def _flutter_ios_simulator_app_impl(ctx):
    bundle = ctx.attr.bundle[0]
    source = bundle[DefaultInfo].files_to_run.executable
    default_info = DefaultInfo(
        files = bundle[DefaultInfo].files,
        runfiles = bundle[DefaultInfo].default_runfiles,
    )
    if source != None:
        runner = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.symlink(output = runner, target_file = source, is_executable = True)
        default_info = DefaultInfo(
            files = depset([runner], transitive = [bundle[DefaultInfo].files]),
            runfiles = ctx.runfiles(files = [runner]).merge(bundle[DefaultInfo].default_runfiles),
            executable = runner,
        )
    providers = [default_info, bundle[AppleBundleInfo]]
    if OutputGroupInfo in bundle:
        providers.append(bundle[OutputGroupInfo])
    return providers

_flutter_ios_simulator_app = rule(
    implementation = _flutter_ios_simulator_app_impl,
    doc = "Builds one Flutter iOS app for the arm64 simulator in debug mode whatever cell flags are set.",
    attrs = {
        "bundle": attr.label(
            doc = "The upstream Flutter iOS application.",
            cfg = _ios_simulator_transition,
            mandatory = True,
            providers = [AppleBundleInfo],
        ),
    },
    executable = True,
)

def flutter_ios_app(name, application, bundle_id, **kwargs):
    """Thin wrapper over flutter_ios_app. Reads runner files from ios/Runner/ in the calling package and builds the arm64 simulator debug app on a macOS host."""
    visibility = kwargs.pop("visibility", None)
    tags = kwargs.get("tags") or []
    testonly = kwargs.get("testonly", False)
    _flutter_ios_app(
        name = name + "_bundle",
        application = application,
        bundle_id = bundle_id,
        visibility = ["//visibility:private"],
        **dx_language_kwargs(dict(kwargs, tags = tags + ["manual"]), compatible_with = FLUTTER_COMPATIBLE_WITH + APPLE_BUILD_HOST)
    )
    _flutter_ios_simulator_app(
        name = name,
        bundle = ":" + name + "_bundle",
        tags = tags,
        testonly = testonly,
        target_compatible_with = APPLE_BUILD_HOST,
        visibility = visibility,
    )
