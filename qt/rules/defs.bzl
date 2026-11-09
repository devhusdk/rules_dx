"""Qt C++ and QML rules over rules_cc and the pinned Qt code generators."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(
    "//libs/starlark:wrapper.bzl",
    "dx_binary_forward_kwargs",
    "dx_forward_data_attr",
    "dx_forwarded_runtime_providers",
    "dx_lcov_merger_attr",
    "dx_quality_sources",
    "dx_symlink_default_info",
    "dx_run_env_attrs",
    "dx_test_forward_kwargs",
    "dx_run_environment",
    "dx_test_upstream_kwargs",
    "dx_upstream_tags",
)
load("//qt/sdk:pins.bzl", "RUNTIME_PLUGINS")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":providers.bzl", "QtInfo")
load(":bundle.bzl", _QT_CONF = "QT_CONF", _qt_bundle = "qt_bundle", _qt_bundle_members = "qt_bundle_members", _qt_os_supplied_libraries = "qt_os_supplied_libraries", _qt_runtime_members = "qt_runtime_members")
load(":cells.bzl", _QT_BUNDLE_COMPATIBLE_WITH = "QT_BUNDLE_COMPATIBLE_WITH", _QT_COMPATIBLE_WITH = "QT_COMPATIBLE_WITH", _QT_CONSUMER_MIN_RUNTIME = "QT_CONSUMER_MIN_RUNTIME", _QT_CONSUMER_RUNTIME = "QT_CONSUMER_RUNTIME", _qt_consumer_identity = "qt_consumer_identity")
load(":constants.bzl", _QT_REPOSITORY = "QT_REPOSITORY", "QT_CC_HDRS", "QT_GENERATED", "QT_QML_SRCS")
load(":text.bzl", _qt_framework_path = "qt_framework_path", _qt_identifier = "qt_identifier", _qt_module_path = "qt_module_path", _qt_package_path = "qt_package_path", _qt_qmldir = "qt_qmldir", _qt_qrc = "qt_qrc", _qt_version_parts = "qt_version_parts")
load(":generate.bzl", _qt_generate = "qt_generate")
load(":rust_bridge.bzl", "QT_BRIDGE_TOOL", _qt_rust_bridge = "qt_rust_bridge", _qt_rust_runtime_files = "qt_rust_runtime_files")

QT_CONF = _QT_CONF
qt_bundle = _qt_bundle
qt_bundle_members = _qt_bundle_members
qt_os_supplied_libraries = _qt_os_supplied_libraries
qt_runtime_members = _qt_runtime_members
QT_BUNDLE_COMPATIBLE_WITH = _QT_BUNDLE_COMPATIBLE_WITH
QT_COMPATIBLE_WITH = _QT_COMPATIBLE_WITH
QT_CONSUMER_MIN_RUNTIME = _QT_CONSUMER_MIN_RUNTIME
QT_CONSUMER_RUNTIME = _QT_CONSUMER_RUNTIME
qt_consumer_identity = _qt_consumer_identity
QT_REPOSITORY = _QT_REPOSITORY
qt_framework_path = _qt_framework_path
qt_identifier = _qt_identifier
qt_module_path = _qt_module_path
qt_package_path = _qt_package_path
qt_qmldir = _qt_qmldir
qt_qrc = _qt_qrc
qt_version_parts = _qt_version_parts

QT_BUNDLE_RUNPATH = "-Wl,--disable-new-dtags,-rpath,$$ORIGIN/resources/lib"
QT_BUNDLE_MACOS_RUNPATH = "-Wl,-rpath,@executable_path/resources/lib"

QT_SOURCE_SPECS = [
    ("c", ["c", "h"]),
    ("cpp", ["cc", "cpp", "cxx", "hh", "hpp", "hxx"]),
    ("qml", ["qml"]),
]

_CC_SRCS = [".c", ".cc", ".cpp", ".cxx"]

_QML_MODULES = ["qml", "quick", "quick_test"]


def _quality_files(ctx):
    return ctx.files.srcs + ctx.files.hdrs + ctx.files.moc_hdrs + ctx.files.qml_srcs

def _qt_library_forward_impl(ctx):
    upstream = ctx.attr.upstream
    generated = ctx.attr.generated
    return [
        upstream[CcInfo],
        DefaultInfo(
            files = depset(transitive = [upstream[DefaultInfo].files, generated[DefaultInfo].files]),
            runfiles = upstream[DefaultInfo].default_runfiles,
        ),
        generated[QtInfo],
    ] + dx_forwarded_runtime_providers(upstream, "qt_*") + [
        dx_quality_sources(_quality_files(ctx), QT_SOURCE_SPECS, str(ctx.label)),
    ]

_SOURCE_ATTRS = {
    "deps": attr.label_list(doc = "Dependencies the QML import context walks."),
    "hdrs": attr.label_list(allow_files = QT_CC_HDRS, doc = "Owned headers."),
    "moc_hdrs": attr.label_list(allow_files = QT_CC_HDRS, doc = "Owned headers moc reads."),
    "qml_srcs": attr.label_list(allow_files = QT_QML_SRCS, doc = "Owned QML sources."),
    "srcs": attr.label_list(allow_files = _CC_SRCS + QT_CC_HDRS, doc = "Owned C++ sources."),
}

_qt_library_forward = rule(
    implementation = _qt_library_forward_impl,
    doc = "Forwards a Qt library and owns only its hand-written sources.",
    provides = [CcInfo, DefaultInfo, InstrumentedFilesInfo, QtInfo, QualitySourcesInfo],
    attrs = dict(_SOURCE_ATTRS, **{
        "generated": attr.label(mandatory = True, providers = [QtInfo], doc = "Qt code generation target."),
        "upstream": attr.label(mandatory = True, providers = [CcInfo], doc = "Private rules_cc library."),
    }),
)

_FRAMEWORK = ".framework/Versions/A/"

def qt_library_stem(basename):
    """Returns the library name without lib prefix and shared-library suffix, or None."""
    name = basename[3:] if basename.startswith("lib") else basename
    for suffix in [".dylib", ".dll", ".so"]:
        if name.endswith(suffix):
            return name[:-len(suffix)]
    head, sep, version = name.rpartition(".so.")
    if sep and version.isdigit():
        return head
    return name if "." not in name else None

def _links_gui(upstream):
    for f in upstream[DefaultInfo].default_runfiles.files.to_list():
        stem = qt_library_stem(f.basename)
        if stem == "Qt6Gui" and f.basename.endswith(".dll"):
            return True
        if stem == "Qt6Gui" and f.basename.endswith(".so.6") and "_solib_" in f.short_path:
            return True
        if stem == "QtGui" and _FRAMEWORK in f.short_path:
            return True
    return False

def _directory(files, stem):
    for f in files:
        if f.basename == stem or qt_library_stem(f.basename) == stem:
            return f.short_path.rpartition("/")[0]
    fail("no " + stem + " in " + str([f.short_path for f in files]))

def _plugin_path(files):
    roots = {}
    for f in files:
        head, sep, tail = f.short_path.rpartition("/plugins/")
        if sep and "/" in tail:
            roots[head + "/plugins"] = True
    if len(roots) != 1:
        fail("qt plugins must share one plugins directory, got " + str(sorted(roots.keys())))
    return roots.keys()[0]

def _run_environment(ctx, gui):
    environment = {"QT_ASSUME_STDERR_HAS_CONSOLE": "1"}
    if len(ctx.files.plugins) > 0:
        environment["QT_PLUGIN_PATH"] = _plugin_path(ctx.files.plugins)
    inherited = []
    if gui:
        if len([f for f in ctx.files.fonts if f.basename == "fonts.conf"]) > 0:
            environment["FONTCONFIG_PATH"] = _directory(ctx.files.fonts, "fonts.conf")
        elif len([f for f in ctx.files.fonts if f.extension == "ttf"]) > 0:
            environment["QT_QPA_FONTDIR"] = _directory(ctx.files.fonts, "DejaVuSans.ttf")
        environment["QT_QPA_PLATFORM_PLUGIN_PATH"] = _directory(ctx.files.platforms, "qoffscreen")
        if ctx.attr._headless:
            environment["QT_QPA_PLATFORM"] = "offscreen"
            environment["QT_QUICK_BACKEND"] = "software"
            environment["QT_FORCE_STDERR_LOGGING"] = "1"
    if RunEnvironmentInfo in ctx.attr.upstream:
        environment.update(ctx.attr.upstream[RunEnvironmentInfo].environment)
        inherited = ctx.attr.upstream[RunEnvironmentInfo].inherited_environment
    roots = [f for f in ctx.files.qml_imports if f.basename == "builtins.qmltypes"]
    if len(ctx.files.qml_imports) > 0 and len(roots) != 1:
        fail(str(ctx.label) + ": qml_imports holds no single builtins.qmltypes at the import root")
    if len(roots) == 1:
        environment["QML_IMPORT_PATH"] = roots[0].short_path.rpartition("/")[0]
    return RunEnvironmentInfo(environment = environment, inherited_environment = inherited)

def _qt_executable_forward_impl(ctx):
    upstream = ctx.attr.upstream
    gui = _links_gui(upstream)
    extra = ctx.files.qml_imports + ctx.files.plugins + (ctx.files.fonts + ctx.files.platforms if gui else [])
    out = [
        dx_symlink_default_info(ctx, "qt_*", ctx.runfiles(files = extra)),
        dx_run_environment(ctx, _run_environment(ctx, gui)),
        dx_quality_sources(_quality_files(ctx), QT_SOURCE_SPECS, str(ctx.label)),
    ]
    if InstrumentedFilesInfo in upstream:
        out.append(upstream[InstrumentedFilesInfo])
    if OutputGroupInfo in upstream:
        out.append(upstream[OutputGroupInfo])
    if CcInfo in upstream:
        out.append(upstream[CcInfo])
    return out

_EXECUTABLE_ATTRS = dict(_SOURCE_ATTRS, **{
    "fonts": attr.label(allow_files = True, doc = "Pinned fontconfig configuration and fonts for Qt Gui."),
    "platforms": attr.label(allow_files = True, doc = "Qt platform plugins for Qt Gui."),
    "plugins": attr.label_list(allow_files = True, doc = "Qt runtime plugin files put on QT_PLUGIN_PATH."),
    "qml_imports": attr.label(allow_files = True, doc = "Qt QML import tree put on QML_IMPORT_PATH."),
    "upstream": attr.label(mandatory = True, providers = [CcInfo], doc = "Private rules_cc executable."),
})

_qt_binary_forward = rule(
    implementation = _qt_executable_forward_impl,
    doc = "Forwards a Qt executable and owns only its hand-written sources.",
    executable = True,
    provides = [DefaultInfo, QualitySourcesInfo, RunEnvironmentInfo],
    attrs = dict(_EXECUTABLE_ATTRS, _headless = attr.bool(default = False), **(dx_forward_data_attr() | dx_run_env_attrs())),
)

_qt_forward_test = rule(
    implementation = _qt_executable_forward_impl,
    doc = "Forwards a Qt test and owns only its hand-written sources.",
    test = True,
    provides = [DefaultInfo, QualitySourcesInfo, RunEnvironmentInfo],
    attrs = dict(_EXECUTABLE_ATTRS, _headless = attr.bool(default = True), **(dx_lcov_merger_attr() | dx_forward_data_attr() | dx_run_env_attrs())),
)

def _qt_frameworks_impl(ctx):
    staged = {}
    for dep in ctx.attr.deps:
        for linker_input in dep[CcInfo].linking_context.linker_inputs.to_list():
            for f in linker_input.additional_inputs:
                relative = qt_framework_path(f.short_path)
                if relative != None and relative not in staged:
                    staged[relative] = f
    if len(staged) == 0:
        return [CcInfo(), DefaultInfo()]
    outputs = []
    for relative in sorted(staged.keys()):
        out = ctx.actions.declare_file(ctx.label.name + "/" + relative)
        ctx.actions.symlink(output = out, target_file = staged[relative])
        outputs.append(out)
    linking_context = cc_common.create_linking_context(linker_inputs = depset([cc_common.create_linker_input(
        owner = ctx.label,
        user_link_flags = ["-Wl,-rpath,@loader_path/" + ctx.label.name],
    )]))
    return [
        CcInfo(linking_context = linking_context),
        DefaultInfo(files = depset(outputs), runfiles = ctx.runfiles(files = outputs)),
    ]

qt_frameworks = rule(
    implementation = _qt_frameworks_impl,
    doc = "Stages the Qt frameworks a binary in the same package links and adds their rpath.",
    provides = [CcInfo],
    attrs = {
        "deps": attr.label_list(providers = [CcInfo], doc = "Qt libraries the binary links."),
    },
)

def _bundle_runpath():
    return select({
        "@platforms//os:macos": [QT_BUNDLE_MACOS_RUNPATH],
        "@platforms//os:windows": [],
        "//conditions:default": [QT_BUNDLE_RUNPATH],
    })

def _no_object_linkopts():
    # Executables that only wrap an alwayslink library link no object files of their own.
    return select({
        "@platforms//os:windows": ["/IGNORE:4001"],
        "//conditions:default": [],
    })

def _link_static(static):
    # Windows DLLs cannot carry an executable's main and macOS dylibs cannot resolve their deps' symbols, so the library goes into the binary.
    return True if static else select({
        "@platforms//os:macos": True,
        "@platforms//os:windows": True,
        "//conditions:default": False,
    })

def _pic_copts():
    return select({
        "@platforms//os:windows": [],
        "//conditions:default": ["-fPIC"],
    })

def _include_copts(name):
    repository = native.repository_name().lstrip("@")
    package = native.package_name()
    parts = (["external", repository] if repository != "" else []) + ([package] if package != "" else [])
    directory = "/".join(parts) if len(parts) > 0 else "."
    return _pic_copts() + ["-I" + directory, "-I$(BINDIR)/" + directory, "-I$(BINDIR)/" + directory + "/" + QT_GENERATED + "/" + name]

def _generate(name, qt, deps, hdrs, moc_hdrs, forms, copts, defines, resources, resource_prefix, translations, qml_srcs, uri, qml_depends, version, tags, testonly, target_compatible_with):
    uses_qml = uri != None
    _qt_generate(
        name = name + "_qt",
        base = name,
        deps = deps,
        hdrs = hdrs,
        copts = copts,
        defines = defines,
        forms = forms,
        lrelease = qt + "//:lrelease" if translations else None,
        metatypes = [qt + "//:metatypes"] if uses_qml else [],
        moc = qt + "//:moc",
        moc_hdrs = moc_hdrs,
        qml_depends = qml_depends,
        qml_srcs = qml_srcs,
        qmltyperegistrar = qt + "//:qmltyperegistrar" if uses_qml else None,
        rcc = qt + "//:rcc",
        resource_prefix = resource_prefix,
        resources = resources,
        translations = translations,
        uic = qt + "//:uic",
        uri = uri if uses_qml else "",
        version = version,
        tags = dx_upstream_tags(tags),
        testonly = testonly,
        target_compatible_with = target_compatible_with,
        visibility = ["//visibility:private"],
    )
    native.filegroup(
        name = name + "_qt_srcs",
        srcs = [":" + name + "_qt"],
        output_group = "qt_srcs",
        tags = dx_upstream_tags(tags),
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    native.filegroup(
        name = name + "_qt_hdrs",
        srcs = [":" + name + "_qt"],
        output_group = "qt_hdrs",
        tags = dx_upstream_tags(tags),
        testonly = testonly,
        visibility = ["//visibility:private"],
    )

def _qt_deps(qt, deps, modules, forms):
    wanted = modules + ["widgets"] if forms and "widgets" not in modules else modules
    return deps + [qt + "//:" + module for module in wanted]

def _gate(kwargs):
    kwargs["target_compatible_with"] = kwargs.get("target_compatible_with", []) + QT_COMPATIBLE_WITH

def _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, qml_srcs, uri, qml_depends, version, copts, visibility, kwargs, owned = True, static = False):
    _gate(kwargs)
    effective_deps = _qt_deps(qt, deps, modules, forms)
    tags = kwargs.pop("tags", None)
    testonly = kwargs.get("testonly", None)
    compatible = kwargs.get("target_compatible_with", None)
    upstream_linkopts = kwargs.pop("linkopts", [])
    _generate(name, qt, effective_deps, hdrs, moc_hdrs, forms, copts + _include_copts(name), kwargs.get("defines", []) + kwargs.get("local_defines", []), resources, resource_prefix, translations, qml_srcs, uri, qml_depends, version, tags, testonly, compatible)
    _cc_library(
        name = name + "_upstream",
        srcs = srcs + [":" + name + "_qt_srcs"],
        hdrs = hdrs + moc_hdrs + [":" + name + "_qt_hdrs"],
        copts = copts + _include_copts(name),
        deps = effective_deps,
        alwayslink = True,
        linkstatic = _link_static(static),
        linkopts = upstream_linkopts,
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
        **kwargs
    )
    forward = {}
    for key in ["aspect_hints", "testonly", "target_compatible_with"]:
        if kwargs.get(key, None) != None:
            forward[key] = kwargs[key]
    if tags != None:
        forward["tags"] = tags
    _qt_library_forward(
        name = name,
        srcs = srcs if owned else [],
        hdrs = hdrs if owned else [],
        moc_hdrs = moc_hdrs if owned else [],
        qml_srcs = qml_srcs,
        deps = deps,
        generated = ":" + name + "_qt",
        upstream = ":" + name + "_upstream",
        visibility = visibility,
        **forward
    )

def qt_cc_library(
        name,
        srcs = None,
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = None,
        modules = ["core"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a C++ library whose moc, uic and rcc outputs are generated from declared inputs."""
    srcs = srcs or []
    deps = deps or []
    _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, [], None, [], "1.0", copts, visibility, kwargs)

def qt_qml_module(
        name,
        uri,
        version = "1.0",
        qml_srcs = [],
        qml_depends = [],
        srcs = None,
        hdrs = [],
        moc_hdrs = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = None,
        modules = ["qml"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QML module with its qmldir, qmltypes, resources and C++ types."""
    srcs = srcs or []
    deps = deps or []
    _library(name, srcs, hdrs, moc_hdrs, [], resources, resource_prefix, translations, deps, modules, qt, qml_srcs, uri, qml_depends, version, copts, visibility, kwargs)

def _uses_qml(modules):
    return len([module for module in modules if module in _QML_MODULES]) > 0

_GUI_MODULES = ["gui", "quick", "widgets"]

_WINDOWS_TEST_CONSOLE = Label("//qt/rules:windows_test_console")

def _windows_console(test, modules):
    # Qt opens a blocking MessageBox for a failed platform plugin when the process has no console window.
    if not test or len([module for module in modules if module in _GUI_MODULES]) == 0:
        return []
    return select({
        "@platforms//os:windows": [_WINDOWS_TEST_CONSOLE],
        "//conditions:default": [],
    })

def _executable(rule_kind, name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, plugins, copts, visibility, kwargs):
    _gate(kwargs)
    for plugin in plugins:
        if plugin not in RUNTIME_PLUGINS:
            fail("qt_cc_" + rule_kind + ": plugin '" + plugin + "' is not one of " + ", ".join(sorted(RUNTIME_PLUGINS.keys())))
    effective_deps = _qt_deps(qt, deps, modules, forms)
    test = rule_kind == "test"
    upstream_kwargs = dx_test_upstream_kwargs(kwargs) if test else dict(kwargs)
    upstream_kwargs.pop("env", None)
    upstream_kwargs.pop("env_inherit", None)
    tags = upstream_kwargs.pop("tags", None)
    upstream_kwargs.pop("visibility", None)
    _generate(name, qt, effective_deps, hdrs, moc_hdrs, forms, copts + _include_copts(name), kwargs.get("defines", []) + kwargs.get("local_defines", []), resources, resource_prefix, translations, [], None, [], "1.0", tags, True if test else kwargs.get("testonly", None), kwargs.get("target_compatible_with", None))
    # Windows has no executable-to-DLL main lookup: a source-less test links its library's main from the static archive.
    upstream_kwargs.setdefault("linkstatic", select({"@platforms//os:windows": True, "//conditions:default": False}))
    upstream_kwargs["linkopts"] = upstream_kwargs.get("linkopts", []) + _no_object_linkopts()
    if not test:
        upstream_kwargs["linkopts"] = upstream_kwargs["linkopts"] + _bundle_runpath()
    qt_frameworks(
        name = name + "_qt_frameworks",
        deps = effective_deps,
        tags = dx_upstream_tags(tags),
        testonly = True if test else kwargs.get("testonly", None),
        target_compatible_with = kwargs.get("target_compatible_with", None),
        visibility = ["//visibility:private"],
    )
    upstream = _cc_test if test else _cc_binary
    upstream(
        name = name + "_upstream",
        srcs = srcs + hdrs + moc_hdrs + [":" + name + "_qt_srcs", ":" + name + "_qt_hdrs"],
        copts = copts + _include_copts(name),
        deps = effective_deps + [":" + name + "_qt_frameworks"] + _windows_console(test, modules),
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward = dx_test_forward_kwargs(kwargs) if test else {}
    if not test:
        forward = dx_binary_forward_kwargs(kwargs)
    forward_rule = _qt_forward_test if test else _qt_binary_forward
    forward_rule(
        name = name,
        srcs = srcs,
        hdrs = hdrs,
        moc_hdrs = moc_hdrs,
        deps = deps,
        fonts = qt + "//:fonts",
        platforms = qt + "//:platforms",
        plugins = [qt + "//:plugins_" + plugin for plugin in plugins],
        qml_imports = qt + "//:qml_imports" if _uses_qml(modules) else None,
        upstream = ":" + name + "_upstream",
        visibility = visibility,
        **forward
    )

def qt_cc_binary(
        name,
        srcs = None,
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = None,
        modules = ["core"],
        qt = QT_REPOSITORY,
        plugins = [],
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a Qt executable over cc_binary."""
    srcs = srcs or []
    deps = deps or []
    _executable("binary", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, plugins, copts, visibility, kwargs)

def qt_cc_test(
        name,
        srcs = None,
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = None,
        modules = ["core", "test"],
        qt = QT_REPOSITORY,
        plugins = [],
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QtTest executable over cc_test."""
    srcs = srcs or []
    deps = deps or []
    _executable("test", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, plugins, copts, visibility, kwargs)


def _output_group(name, target, group, testonly = None):
    native.filegroup(
        name = name,
        srcs = [target],
        output_group = group,
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )

def qt_rust_runtime(name, core, **kwargs):
    """Declares the CXX-Qt runtime library over one QtCore target."""
    _qt_rust_runtime_files(
        name = name + "_files",
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _output_group(name + "_hdrs", ":" + name + "_files", "qt_rust_hdrs")
    _output_group(name + "_srcs", ":" + name + "_files", "qt_rust_srcs")
    _cc_library(
        name = name,
        srcs = [":" + name + "_srcs"],
        hdrs = [":" + name + "_hdrs"],
        copts = _pic_copts(),
        includes = [name + "_files"],
        deps = [core],
        alwayslink = True,
        linkstatic = True,
        linkopts = kwargs.pop("linkopts", []),
        **kwargs
    )

def qt_rust_cc_library(
        name,
        bridges,
        rust,
        cc_deps = [],
        generator = None,
        uri = None,
        version = "1.0",
        qml_srcs = [],
        qml_depends = [],
        modules = None,
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares the Qt C++ library generated from CXX-Qt bridges of one Rust crate."""
    _gate(kwargs)
    testonly = kwargs.get("testonly", None)
    shared = {}
    for key in ["testonly", "target_compatible_with"]:
        if kwargs.get(key, None) != None:
            shared[key] = kwargs[key]
    _qt_rust_bridge(
        name = name + "_bridge",
        bridges = bridges,
        generator = generator or QT_BRIDGE_TOOL,
        tags = (kwargs.get("tags", None) or []) + ["manual"],
        visibility = visibility,
        **shared
    )
    _output_group(name + "_bridge_hdrs", ":" + name + "_bridge", "qt_rust_hdrs", testonly)
    _output_group(name + "_bridge_moc", ":" + name + "_bridge", "qt_rust_moc_hdrs", testonly)
    _output_group(name + "_bridge_srcs", ":" + name + "_bridge", "qt_rust_srcs", testonly)
    _library(
        name,
        [":" + name + "_bridge_srcs"],
        [":" + name + "_bridge_hdrs"],
        [":" + name + "_bridge_moc"],
        [],
        [],
        "/",
        [],
        cc_deps + [rust, qt + "//:cxx_qt"],
        modules if modules != None else (["qml"] if uri != None else ["core"]),
        qt,
        qml_srcs,
        uri,
        qml_depends,
        version,
        copts,
        visibility,
        kwargs,
        owned = False,
        static = True,
    )
