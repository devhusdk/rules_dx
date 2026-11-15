"""qt_consumer_archive and qt_bundle_runtime: consumer_archive for the targeted Qt cell."""

load("//qt/private:bundle.bzl", "QT_CONF", "qt_runtime_members")
load("//qt/private:cells.bzl", "QT_BUNDLE_COMPATIBLE_WITH", "qt_consumer_identity")
load("//qt/private:constants.bzl", "QT_REPOSITORY")
load("//qt/private/toolchain:pins.bzl", "CELLS", "IDENTITIES", "QT_VERSION")
load("//qt/private/toolchain:repos.bzl", "qualified_cells")
load(":archive.bzl", "consumer_archive")
load(":native_runtime.bzl", "NativeRuntimeInfo")

_CONSUMER_CELLS = [cell for cell in qualified_cells() if IDENTITIES[cell] != None]

_BUNDLE_CELLS = [cell for cell in _CONSUMER_CELLS if CELLS[cell].needs != None]

QT_OS_RUNTIME_CELLS = [cell for cell in _BUNDLE_CELLS if CELLS[cell].layout.os == "linux"]

_WINDOWS_CELLS = [cell for cell in _BUNDLE_CELLS if CELLS[cell].layout.os == "windows"]

def _cell_setting(cell):
    return Label("//qt/private/platforms:cell_" + cell)

QT_CONSUMER_MIN_RUNTIME = select(
    {_cell_setting(cell): qt_consumer_identity(cell).min_runtime for cell in _CONSUMER_CELLS} |
    {"//conditions:default": ""},
)

QT_CONSUMER_RUNTIME = select(
    {_cell_setting(cell): [Label("//deploy/private/qt:os_runtime_" + cell)] for cell in QT_OS_RUNTIME_CELLS} |
    {_cell_setting(cell): [Label("//cc/private/toolchain:msvc_vcruntime")] for cell in _WINDOWS_CELLS} |
    {"//conditions:default": []},
)

def _qt_bundle_runtime_impl(ctx):
    prefix = "/" + ctx.attr.bundle.label.name + "/"
    dlls = [f for f in ctx.attr.bundle[DefaultInfo].files.to_list() if qt_runtime_members([f.path.rpartition(prefix)[2]])]
    if len(dlls) == 0:
        return [
            DefaultInfo(),
            NativeRuntimeInfo(name = ctx.label.name, version = QT_VERSION, source = "Qt", redistribution = "os_supplied", files = [], notice = None, provides = []),
        ]
    conf = ctx.actions.declare_file(ctx.label.name + "/qt.conf")
    ctx.actions.write(conf, QT_CONF)
    files = dlls + [conf]
    return [
        DefaultInfo(files = depset(files)),
        NativeRuntimeInfo(name = ctx.label.name, version = QT_VERSION, source = "Qt", redistribution = "app_local", files = files, notice = ctx.files.license_files[0], provides = []),
    ]

_qt_bundle_runtime = rule(
    implementation = _qt_bundle_runtime_impl,
    doc = "The Qt DLLs of a qt_bundle and a qt.conf for consumer_archive runtime; empty where the bundle has no DLLs.",
    attrs = {
        "bundle": attr.label(mandatory = True, providers = [DefaultInfo], doc = "qt_bundle to read the DLLs from."),
        "license_files": attr.label_list(allow_files = True, doc = "License texts, the first is the notice."),
    },
)

def qt_bundle_runtime(name, bundle, qt = QT_REPOSITORY, **kwargs):
    """Declares the Qt DLLs of a qt_bundle and a qt.conf as one consumer_archive runtime."""
    compatible = kwargs.pop("target_compatible_with", None)
    _qt_bundle_runtime(
        name = name,
        bundle = bundle,
        license_files = [qt + "//:licenses"],
        target_compatible_with = (compatible or []) + QT_BUNDLE_COMPATIBLE_WITH,
        **kwargs
    )

def qt_consumer_archive(name, app, min_runtime = None, runtime = [], **kwargs):
    """Packages a qt_cc_binary with consumer_archive for the targeted Qt cell."""
    consumer_archive(
        name = name,
        app = app,
        min_runtime = min_runtime or QT_CONSUMER_MIN_RUNTIME,
        runtime = QT_CONSUMER_RUNTIME + runtime,
        **kwargs
    )
