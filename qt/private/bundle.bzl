"""qt_bundle: the Qt runtime of one binary as a deployable archive."""

load("//deploy/private:native_runtime.bzl", "NativeRuntimeInfo")
load("//qt/private/sdk:needs.bzl", "QtNeedsInfo")
load("//qt/private/sdk:pins.bzl", "QT_VERSION", "RUNTIME_PLUGINS")
load(":cells.bzl", "QT_BUNDLE_COMPATIBLE_WITH")
load(":constants.bzl", "QT_REPOSITORY")
load(":text.bzl", "qt_framework_path", "qt_module_path")

def _soname(basename):
    stem, separator, version = basename.rpartition(".so.")
    return separator != "" and stem != "" and version.isdigit()

def qt_os_supplied_libraries(library):
    """Returns the sorted system libraries a Qt library table needs, which the bundle leaves to the OS."""
    names = {}
    for needed in library.values():
        for name in needed:
            if not name.startswith("libQt6") and not name.startswith("libicu"):
                names[name] = True
    return sorted(names)

QT_CONF = "[Paths]\nPrefix = resources\n"

def qt_runtime_members(members):
    """Returns the bundle members that sit next to the executable: the Windows DLLs."""
    return [member for member in members if member.startswith("bin/") and member.endswith(".dll")]

def qt_bundle_members(runfiles, licenses, modules, needs):
    """Returns the sorted (member, short path) pairs one bundle stages."""
    roots = [path.rpartition("/")[0] for path in runfiles if path.rpartition("/")[2] == "builtins.qmltypes"]
    if len(roots) > 1:
        fail("qt_bundle: the binary carries " + str(len(roots)) + " QML import trees")
    root = roots[0] + "/" if len(roots) == 1 else None
    if len(modules) > 0 and root == None:
        fail("qt_bundle: qml_modules " + str(modules) + " need a binary with the QML import tree")
    members = {}
    shipped = {}
    libraries = {}
    frameworks = {}
    roots = []
    ordered = sorted([("_solib_" in path, path) for path in runfiles])
    for linked, path in ordered:
        if root != None and path.startswith(root):
            relative = path[len(root):]
            for module in modules:
                if relative.startswith(qt_module_path(module) + "/"):
                    members["qml/" + relative] = path
                    shipped[module] = True
                    roots.extend(needs.plugin.get(relative, []))
            continue
        framework = qt_framework_path(path)
        if framework != None:
            name = framework.partition("/")[0]
            if framework == name + "/Versions/A/" + name.removesuffix(".framework"):
                if name not in frameworks:
                    frameworks[name] = framework
                    libraries[name] = path
                if not path.startswith("../"):
                    roots.append(name)
            continue
        name = path.rpartition("/")[2]
        if path.rpartition("/")[0].endswith("/fonts") and (name == "fonts.conf" or name.endswith(".ttf")):
            members["fonts/" + name] = path
            continue
        plugin = [key for key in needs.runtime_plugin if path.endswith("/plugins/" + key)]
        if len(plugin) == 1:
            members["plugins/" + plugin[0]] = path
            roots.extend(needs.runtime_plugin[plugin[0]])
            continue
        if _soname(name) or name.endswith(".so") or name.endswith(".dylib"):
            libraries.setdefault(name, path)
            if linked:
                roots.append(name)
        elif name.endswith(".dll"):
            libraries.setdefault(name, path)
            if not path.startswith("../"):
                roots.append(name)
    needed = {}
    for _ in range(len(libraries) + 1):
        pending = [name for name in roots if name in libraries and name not in needed]
        if len(pending) == 0:
            break
        for name in pending:
            needed[name] = True
            roots.extend(needs.library.get(name, []))
    for name in needed:
        if name.endswith(".dll"):
            members["bin/" + name] = libraries[name]
            continue
        if name in frameworks:
            members["lib/" + frameworks[name]] = libraries[name]
            continue
        versioned = [other for other in libraries if other.startswith(name + ".") and _soname(other)]
        if _soname(name) or len(versioned) == 0:
            members["lib/" + name] = libraries[name]
    for module in modules:
        if module not in shipped:
            fail("qt_bundle: QML module '" + module + "' is not in the import tree")
    for path in licenses:
        relative = ("/" + path).partition("/licenses/")[2]
        if relative == "":
            fail("qt_bundle: license '" + path + "' is not below a licenses directory")
        members["licenses/" + relative] = path
    return sorted(members.items())

def _qt_bundle_impl(ctx):
    runfiles = ctx.attr.binary[DefaultInfo].default_runfiles.files.to_list() + ctx.files.display_plugins + ctx.files.runtime_plugins
    files = {f.short_path: f for f in runfiles + ctx.files.license_files}
    modules = ctx.attr.qml_modules
    staged = []
    for member, path in qt_bundle_members([f.short_path for f in runfiles], [f.short_path for f in ctx.files.license_files], modules, ctx.attr.needs[QtNeedsInfo]):
        out = ctx.actions.declare_file(ctx.label.name + "/" + member)
        ctx.actions.symlink(output = out, target_file = files[path])
        staged.append(out)
    return [DefaultInfo(files = depset(staged))]

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

_qt_bundle = rule(
    implementation = _qt_bundle_impl,
    doc = "Stages the linked shared libraries (Windows DLLs under bin/), their dependencies, QML modules, plugins and licenses one Qt binary runs with.",
    attrs = {
        "binary": attr.label(mandatory = True, providers = [RunEnvironmentInfo], doc = "qt_cc_binary to bundle."),
        "display_plugins": attr.label_list(allow_files = True, doc = "Display platform plugin files to ship."),
        "license_files": attr.label_list(allow_files = True, doc = "License texts below a licenses directory."),
        "needs": attr.label(mandatory = True, providers = [QtNeedsInfo], doc = "Dependency tables of the Qt cell the binary runs on."),
        "qml_modules": attr.string_list(doc = "QML import modules to ship."),
        "runtime_plugins": attr.label_list(allow_files = True, doc = "Qt runtime plugin files to ship."),
    },
)

def qt_bundle(name, binary, qml_modules = [], licenses = [], display = [], plugins = [], qt = QT_REPOSITORY, **kwargs):
    """Stages a qt_cc_binary runtime for consumer_archive resources."""
    for platform in display:
        if platform not in ["wayland", "xcb"]:
            fail("qt_bundle: display '" + platform + "' is not one of wayland, xcb")
    for plugin in plugins:
        if plugin not in RUNTIME_PLUGINS:
            fail("qt_bundle: plugin '" + plugin + "' is not one of " + ", ".join(sorted(RUNTIME_PLUGINS.keys())))
    compatible = kwargs.pop("target_compatible_with", None)
    kwargs["target_compatible_with"] = (compatible or []) + QT_BUNDLE_COMPATIBLE_WITH
    _qt_bundle(
        name = name,
        binary = binary,
        display_plugins = select({
            "@platforms//os:macos": [qt + "//:display_cocoa"] if len(display) > 0 else [],
            "@platforms//os:windows": [],
            "//conditions:default": [qt + "//:display_" + platform for platform in display],
        }),
        license_files = [qt + "//:licenses"] + licenses,
        needs = qt + "//:needs",
        qml_modules = qml_modules,
        runtime_plugins = [qt + "//:plugins_" + plugin for plugin in plugins],
        **kwargs
    )
    _qt_bundle_runtime(
        name = name + "_runtime",
        bundle = name,
        license_files = [qt + "//:licenses"],
        target_compatible_with = kwargs["target_compatible_with"],
        visibility = kwargs.get("visibility"),
    )
