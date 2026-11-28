"""Thin Flutter wrappers over rules_flutter."""

load("@rules_apple//apple:providers.bzl", "AppleBundleInfo")
load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("@rules_flutter//flutter:android.bzl", _flutter_android_app = "flutter_android_app", _flutter_android_build = "flutter_android_build")
load("@rules_flutter//flutter:defs.bzl", _flutter_application = "flutter_application", _flutter_font = "flutter_font", _flutter_font_family = "flutter_font_family", _flutter_gen_l10n = "flutter_gen_l10n", _flutter_library = "flutter_library", _flutter_test = "flutter_test")
load("@rules_flutter//flutter:ios.bzl", _flutter_ios_app = "flutter_ios_app")
load("@rules_flutter//flutter:providers.bzl", "FlutterApplicationInfo", "FlutterApplicationInputsInfo", "FlutterInfo")
load("@rules_flutter//flutter:web.bzl", _flutter_web_app = "flutter_web_app", _flutter_web_bundle = "flutter_web_bundle")
load("//apple/private/bundle:defs.bzl", "EXTRA_TOOLCHAINS_SETTING", "apple_extra_toolchains")
load("//apple/private/ios:cell.bzl", "APPLE_BUILD_HOST")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:platform_support.bzl", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_kwargs", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//dart/private:defs.bzl", "DART_QUALITY_TOOLS", "DART_SOURCE_SPECS")
load("//flutter/private:platforms.bzl", FLUTTER_SUPPORT = "PLATFORM_SUPPORT")

flutter_gen_l10n = _flutter_gen_l10n

flutter_font = _flutter_font

flutter_font_family = _flutter_font_family

FLUTTER_SOURCE_EXTS = [".dart"]

FLUTTER_COMPATIBLE_WITH = compatible_with(FLUTTER_SUPPORT)

_FLUTTER_ANDROID_PLATFORMS = {
    "arm64": Label("@rules_android//:arm64-v8a"),
    "x64": Label("@rules_android//:x86_64"),
}

FLUTTER_ANDROID_ABIS = sorted(_FLUTTER_ANDROID_PLATFORMS.keys())

_FLUTTER = dx_language_spec(what = "flutter_*", quality_specs = DART_SOURCE_SPECS, allow_files = FLUTTER_SOURCE_EXTS, quality_tools = DART_QUALITY_TOOLS)

_flutter_library_forward = dx_library_forward_rule(
    _FLUTTER,
    dx_forward_spec(
        provides = [DartAnalyzableInfo, DartInfo, DefaultInfo, FlutterInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
        required_providers = [(DartInfo, "DartInfo"), (DartAnalyzableInfo, "DartAnalyzableInfo"), (FlutterInfo, "FlutterInfo")],
        upstream_providers = [[DartInfo, DartAnalyzableInfo, FlutterInfo]],
        runtime = "besteffort",
    ),
)

_flutter_forward_test = dx_executable_forward_rule(
    _FLUTTER,
    "test",
    dx_forward_spec(
        provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, QualityToolsInfo],
        required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
        upstream_providers = [[DartAnalyzableInfo]],
        extra_attrs = dx_lcov_merger_attr(),
    ),
)

_flutter_web_forward = dx_library_forward_rule(
    _FLUTTER,
    dx_forward_spec(
        provides = [DartAnalyzableInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
        required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
        upstream_providers = [[DartAnalyzableInfo]],
        runtime = "besteffort",
    ),
)

_flutter_application_forward = dx_library_forward_rule(
    _FLUTTER,
    dx_forward_spec(
        provides = [DefaultInfo, FlutterApplicationInputsInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
        required_providers = [(FlutterApplicationInputsInfo, "FlutterApplicationInputsInfo")],
        upstream_providers = [[FlutterApplicationInputsInfo]],
        quiet_providers = [DartAnalyzableInfo, FlutterApplicationInfo],
        runtime = "besteffort",
    ),
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

def flutter_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_library."""
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
        **kwargs
    )

def flutter_web_app(name, *, application, visibility = None, **kwargs):
    """Thin wrapper over flutter_web_app, the web bundle of a flutter_application."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    dx_wrap_binary(name, _flutter_web_app, _flutter_web_forward, None, visibility = visibility, upstream_kwargs = dict(kwargs, application = application), **kwargs)

def flutter_web_bundle(name, *, application, **kwargs):
    """Thin wrapper over flutter_web_bundle, for dart2js or canvaskit builds."""
    _flutter_web_bundle(name = name, application = application, **dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH))

def flutter_android_app(name, application, application_id, android_abi = "arm64", visibility = None, **kwargs):
    """Thin wrapper over flutter_android_app. An unknown android_abi fails when the target is analyzed."""
    if android_abi not in FLUTTER_ANDROID_ABIS:
        _declare_invalid(name, "flutter_android_app: android_abi '{}' is unknown: want {}".format(android_abi, ", ".join(FLUTTER_ANDROID_ABIS)), kwargs, visibility)
        return
    app_kwargs = dx_language_kwargs(kwargs, compatible_with = FLUTTER_COMPATIBLE_WITH)
    tags = app_kwargs.get("tags") or []
    _flutter_android_app(
        name = name + "_apk",
        application = application,
        application_id = application_id,
        visibility = ["//visibility:private"],
        **dict(app_kwargs, tags = tags + ["manual"])
    )
    _flutter_android_build(
        name = name,
        android_platforms = [_FLUTTER_ANDROID_PLATFORMS[android_abi]],
        targets = [":" + name + "_apk"],
        tags = tags,
        testonly = app_kwargs.get("testonly"),
        target_compatible_with = app_kwargs.get("target_compatible_with"),
        visibility = visibility,
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
    """Thin wrapper over flutter_ios_app."""
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
