"""Thin Flutter wrappers over rules_flutter."""

load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("@rules_dart//dart:utils.bzl", "dart_analyzable_info_with_package", "derive_lib_root")
load("@rules_flutter//flutter:android.bzl", _flutter_android_app = "flutter_android_app")
load("@rules_flutter//flutter:defs.bzl", _flutter_application = "flutter_application", _flutter_gen_l10n = "flutter_gen_l10n", _flutter_library = "flutter_library", _flutter_test = "flutter_test")
load("@rules_flutter//flutter:ios.bzl", _flutter_ios_app = "flutter_ios_app")
load("@rules_flutter//flutter:providers.bzl", "FlutterApplicationInfo", "FlutterInfo")
load("@rules_flutter//flutter:web.bzl", _flutter_web_app = "flutter_web_app", _flutter_web_bundle = "flutter_web_bundle")
load("//dart:defs.bzl", "DART_QUALITY_TOOLS", "DART_SOURCE_SPECS")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:platform_support.bzl", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//flutter/private:platforms.bzl", FLUTTER_SUPPORT = "PLATFORM_SUPPORT")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

flutter_gen_l10n = _flutter_gen_l10n

FLUTTER_SOURCE_EXTS = [".dart"]

FLUTTER_DEFERRED_SKIP_TAGS = []

FLUTTER_COMPATIBLE_WITH = compatible_with(FLUTTER_SUPPORT)

FLUTTER_ANDROID_ABIS = ["arm64", "x64"]

_flutter_library_forward = dx_library_forward_rule(
    provides = [DartAnalyzableInfo, DartInfo, DefaultInfo, FlutterInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartInfo, "DartInfo"), (DartAnalyzableInfo, "DartAnalyzableInfo"), (FlutterInfo, "FlutterInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_library",
    allow_files = FLUTTER_SOURCE_EXTS,
    upstream_providers = [[DartInfo, DartAnalyzableInfo, FlutterInfo]],
    runtime = "besteffort",
)

_flutter_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_test",
    allow_files = FLUTTER_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

_flutter_web_forward = dx_library_forward_rule(
    provides = [DartAnalyzableInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_web_app",
    allow_files = FLUTTER_SOURCE_EXTS,
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
    provides = [DartAnalyzableInfo, DefaultInfo, FlutterApplicationInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(FlutterApplicationInfo, "FlutterApplicationInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_application",
    allow_files = FLUTTER_SOURCE_EXTS,
    upstream_providers = [[FlutterApplicationInfo]],
    extra_attrs = {
        "deps": attr.label_list(doc = "Dart libraries the application imports.", providers = [DartInfo]),
        "language_version": attr.string(doc = "Dart language version of the application."),
        "package_name": attr.string(doc = "Dart package name of the application.", mandatory = True),
    },
    extra_providers = _application_dart_info,
    runtime = "besteffort",
)

def _missing_font_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".missing_font")

    # run_shell: The action only prints the error to stderr and fails.
    ctx.actions.run_shell(
        outputs = [out],
        command = "echo 'flutter_library {}: fonts_json names font asset \"{}\" but font_files does not provide it' >&2; exit 1".format(ctx.label.name, ctx.attr.asset),
    )
    return [DefaultInfo(files = depset([out]))]

_missing_font = rule(
    implementation = _missing_font_impl,
    doc = "Fails the build because a font asset named in fonts_json has no file.",
    attrs = {"asset": attr.string(doc = "Font asset path that has no file.")},
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
        _missing_font(name = name, asset = missing, tags = kwargs.get("tags", []), visibility = visibility)
        return
    dx_wrap(name, _flutter_library, _flutter_library_forward, srcs or [], visibility = visibility, **dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH))

def flutter_test(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_test. main is name.dart, then test/name.dart, else the single src."""
    main = dx_infer_main("flutter_test", name, main, srcs, dx_main_candidates(name, ".dart", ["test"]))
    kwargs = dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH)
    dx_wrap_test(name, _flutter_test, _flutter_forward_test, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def flutter_application(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_application. main is name.dart, then lib/main.dart, else the single src."""
    main = dx_infer_main("flutter_application", name, main, srcs, dx_main_candidates(name, ".dart") + ["lib/main.dart"])
    kwargs = dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH)
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

def flutter_web_app(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_web_app. main is name.dart, then lib/main.dart, else the single src."""
    main = dx_infer_main("flutter_web_app", name, main, srcs, dx_main_candidates(name, ".dart") + ["lib/main.dart"])
    kwargs = dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH)
    dx_wrap_binary(name, _flutter_web_app, _flutter_web_forward, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def flutter_web_bundle(name, **kwargs):
    """Thin wrapper over flutter_web_bundle, for dart2js or canvaskit builds."""
    _flutter_web_bundle(name = name, **dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH))

def _bad_abi_impl(ctx):
    fail(ctx.attr.message)

_bad_abi = rule(
    implementation = _bad_abi_impl,
    doc = "Fails the build with the given ABI message.",
    attrs = {"message": attr.string(doc = "Failure message.", mandatory = True)},
)

def _flutter_app_kwargs(kwargs):
    app_kwargs = dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH)
    if not app_kwargs.get("tags"):
        app_kwargs["tags"] = ["manual"]
    return app_kwargs

def flutter_android_app(name, application, package_name, android_abi = "arm64", visibility = None, **kwargs):
    """Thin wrapper over flutter_android_app. An unknown android_abi fails when the target is analyzed."""
    if android_abi not in FLUTTER_ANDROID_ABIS:
        checked = dx_language_kwargs(kwargs, FLUTTER_DEFERRED_SKIP_TAGS, FLUTTER_COMPATIBLE_WITH)
        _bad_abi(
            name = name,
            message = "flutter_android_app: android_abi '{}' is unknown: want {}".format(android_abi, ", ".join(FLUTTER_ANDROID_ABIS)),
            tags = checked.get("tags", []),
            target_compatible_with = checked["target_compatible_with"],
            visibility = visibility,
        )
        return
    app_kwargs = _flutter_app_kwargs(kwargs)
    _flutter_android_app(
        name = name,
        application = application,
        package_name = package_name,
        android_abi = android_abi,
        visibility = visibility,
        **app_kwargs
    )

def flutter_ios_app(name, application, bundle_id, **kwargs):
    """Thin wrapper over flutter_ios_app. Reads runner files from ios/Runner/ in the calling package and builds only for iOS."""
    _flutter_ios_app(
        name = name,
        application = application,
        bundle_id = bundle_id,
        **_flutter_app_kwargs(kwargs)
    )
