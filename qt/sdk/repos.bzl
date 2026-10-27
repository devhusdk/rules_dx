"""Opt-in hermetic Qt SDK acquisition."""

load(
    ":pins.bzl",
    "CELLS",
    "CLOSURES",
    "DEFAULT_MODULES",
    "EXTRACTORS",
    "LICENSE_TEXTS",
    "MODULES",
    "QT_MIRROR",
    "QT_VERSION",
    "RUNTIME_MIRROR",
    "RUNTIME_REPOSITORIES",
)

HUB_NAME = "dx_qt"

SHARED_TARGETS = ["licenses", "runtime", "sbom"]

TOOLS = {
    "lrelease": struct(data = [], path = "bin/lrelease"),
    "moc": struct(data = [], path = "libexec/moc"),
    "qmlformat": struct(data = [":plugins", ":qml_imports"], path = "bin/qmlformat"),
    "qmllint": struct(data = [":plugins", ":qml_imports"], path = "bin/qmllint"),
    "qmltyperegistrar": struct(data = [], path = "libexec/qmltyperegistrar"),
    "rcc": struct(data = [], path = "libexec/rcc"),
    "uic": struct(data = [], path = "libexec/uic"),
}

_CELL_PREFIX = "dx_qt_"

_EXTRACT = "_extract"

_HEX = "0123456789abcdef"

_HOST_OSES = {"linux": "linux", "mac os x": "macos"}

_HOST_ARCHES = {"aarch64": "arm64", "amd64": "x86_64", "arm64": "arm64", "x86_64": "x86_64"}

_QML = [":qml", ":network", ":core"]

_CLOSURE_DEPS = [":closure", ":gui_closure"]

_LAUNCHERS = {"posix": "_launcher"}

_LIBRARIES = {
    "core": struct(define = "QT_CORE_LIB", deps = [":closure"], headers = ["QtCore"], module = "qtbase", name = "Core"),
    "dbus": struct(define = "QT_DBUS_LIB", deps = [":core"], headers = ["QtDBus"], module = "qtbase", name = "DBus"),
    "gui": struct(define = "QT_GUI_LIB", deps = [":dbus", ":core", ":gui_closure"], headers = ["QtGui"], module = "qtbase", name = "Gui"),
    "network": struct(define = "QT_NETWORK_LIB", deps = [":core"], headers = ["QtNetwork"], module = "qtbase", name = "Network"),
    "opengl": struct(define = "QT_OPENGL_LIB", deps = [":gui", ":core"], headers = ["QtOpenGL"], module = "qtbase", name = "OpenGL"),
    "qml": struct(define = "QT_QML_LIB", deps = [":network", ":core"], headers = ["QtQml", "QtQmlIntegration"], module = "qtdeclarative", name = "Qml"),
    "qml_meta": struct(define = "QT_QMLMETA_LIB", deps = [":qml_models", ":qml_worker_script"] + _QML, headers = ["QtQmlMeta"], module = "qtdeclarative", name = "QmlMeta"),
    "qml_models": struct(define = "QT_QMLMODELS_LIB", deps = _QML, headers = ["QtQmlModels"], module = "qtdeclarative", name = "QmlModels"),
    "qml_worker_script": struct(define = "QT_QMLWORKERSCRIPT_LIB", deps = _QML, headers = ["QtQmlWorkerScript"], module = "qtdeclarative", name = "QmlWorkerScript"),
    "quick": struct(define = "QT_QUICK_LIB", deps = [":qml_meta", ":opengl", ":gui"] + _QML, headers = ["QtQuick"], module = "qtdeclarative", name = "Quick"),
    "quick_test": struct(define = "QT_QUICKTEST_LIB", deps = [":quick", ":test"], headers = ["QtQuickTest"], module = "qtdeclarative", name = "QuickTest"),
    "test": struct(define = "QT_TESTLIB_LIB", deps = [":core"], headers = ["QtTest"], module = "qtbase", name = "Test"),
    "widgets": struct(define = "QT_WIDGETS_LIB", deps = [":gui", ":core"], headers = ["QtWidgets"], module = "qtbase", name = "Widgets"),
}

_FONTS_CONF = """<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <dir prefix="relative">.</dir>
</fontconfig>
"""

_LAUNCH = """int dx_qt_launch(int argc, char **argv, const char *repository, const char *tool);

int main(int argc, char **argv) {
  return dx_qt_launch(argc, argv, "%s", "%s");
}
"""

def cell_repository_name(cell):
    """Returns the repository name one qualified cell is acquired into."""
    return _CELL_PREFIX + cell

def qualified_cells():
    """Returns the cells with pinned archives and probe evidence."""
    return [cell for cell in sorted(CELLS.keys()) if CELLS[cell].blocker == ""]

def hub_targets():
    """Returns every target name the hub repository exposes."""
    names = list(SHARED_TARGETS)
    for module in MODULES.values():
        names.extend(module.targets)
    return sorted(names)

def _is_sha256(value):
    if len(value) != 64:
        return False
    for index in range(len(value)):
        if value[index] not in _HEX:
            return False
    return True

def _archive_errors(cell, key, archive, release):
    errors = []
    if not archive.archive.startswith(release):
        errors.append("cell '" + cell + "' archive '" + key + "' is not from release " + release)
    if not archive.archive.endswith(".7z"):
        errors.append("cell '" + cell + "' archive '" + key + "' is not a .7z archive")
    if not _is_sha256(archive.sha256):
        errors.append("cell '" + cell + "' archive '" + key + "' sha256 is not 64 hex characters")
    return errors

def host_key(os, arch):
    """Returns the EXTRACTORS key of one host from Bazel's os name and arch."""
    name = os.lower()
    name = "windows" if name.startswith("windows") else _HOST_OSES.get(name, name)
    return name + "_" + _HOST_ARCHES.get(arch, arch)

def cell_errors(cell, entry):
    """Returns one error string per pin of one cell entry that is not ready to acquire."""
    errors = []
    if len(entry.constraints) == 0:
        errors.append("cell '" + cell + "' names no platform constraints")
    if entry.blocker != "":
        if len(entry.archives) > 0 or entry.directory != "" or entry.release != "" or entry.runtime != None:
            errors.append("cell '" + cell + "' is blocked but names archives")
        return errors
    if entry.directory == "":
        errors.append("cell '" + cell + "' names no repository directory")
    if not entry.release.startswith(QT_VERSION + "-"):
        errors.append("cell '" + cell + "' release '" + entry.release + "' is not a " + QT_VERSION + " release")
    if entry.layout == None:
        errors.append("cell '" + cell + "' names no layout")
        return errors
    if entry.layout.launcher not in _LAUNCHERS:
        errors.append("cell '" + cell + "' layout launcher '" + entry.layout.launcher + "' is not one of " + ", ".join(sorted(_LAUNCHERS.keys())))
    if entry.runtime == None and entry.layout.os == "linux":
        errors.append("cell '" + cell + "' pins no runtime closure")
    for key in _archive_keys(entry) + sorted(MODULES.keys()):
        if key not in entry.archives:
            errors.append("cell '" + cell + "' pins no '" + key + "' archive")
            continue
        errors.extend(_archive_errors(cell, key, entry.archives[key], entry.release))
    return errors

def _archive_keys(entry):
    return ["icu"] if len(entry.layout.icu_libraries) > 0 else []

def _rpm_errors(package, arch):
    errors = []
    if not package.file.startswith(package.name + "-") or not package.file.endswith("." + arch + ".rpm"):
        errors.append("runtime package '" + package.name + "' file '" + package.file + "' is not its " + arch + " rpm")
    if not _is_sha256(package.sha256):
        errors.append("runtime package '" + package.name + "' sha256 is not 64 hex characters")
    if package.repository not in RUNTIME_REPOSITORIES:
        errors.append("runtime package '" + package.name + "' repository '" + package.repository + "' is not one of " + ", ".join(RUNTIME_REPOSITORIES))
    if package.license == "":
        errors.append("runtime package '" + package.name + "' names no license")
    return errors

def _package_errors(runtime, package, names):
    errors = _rpm_errors(package, runtime.arch)
    if len(package.libraries) == 0:
        errors.append("runtime package '" + package.name + "' carries no libraries")
    if package.closure not in CLOSURES:
        errors.append("runtime package '" + package.name + "' closure '" + package.closure + "' is not one of " + ", ".join(CLOSURES))
    if package.license_from == "":
        texts = [key for key in runtime.license_texts.keys() if key.startswith(package.name + "/")]
        if package.license_dir != "" or len(texts) == 0:
            errors.append("runtime package '" + package.name + "' has no license text")
    elif package.license_from == package.name and package.license_dir == "":
        errors.append("runtime package '" + package.name + "' names no license directory")
    elif package.license_from not in names:
        errors.append("runtime package '" + package.name + "' takes its license from an unknown package")
    return errors

def pin_errors():
    """Returns one error string per pin that is not ready to acquire."""
    errors = []
    if len(qualified_cells()) == 0:
        errors.append("no cell is qualified")
    for cell in sorted(CELLS.keys()):
        errors.extend(cell_errors(cell, CELLS[cell]))
    for name in sorted(MODULES.keys()):
        for required in MODULES[name].requires:
            if required not in MODULES:
                errors.append("module '" + name + "' requires unknown module '" + required + "'")
    texts = dict(LICENSE_TEXTS)
    for cell in qualified_cells():
        runtime = CELLS[cell].runtime
        if runtime == None:
            continue
        names = [package.name for package in runtime.packages]
        if names != sorted(names):
            errors.append("cell '" + cell + "' runtime packages are not sorted by name")
        for package in runtime.packages:
            errors.extend(_package_errors(runtime, package, names))
        errors.extend(_rpm_errors(runtime.font, "noarch"))
        if len(runtime.font.fonts) == 0 or runtime.font.license_dir == "":
            errors.append("font package '" + runtime.font.name + "' carries no fonts or license directory")
        for key in runtime.license_texts:
            texts[cell + ": " + key] = runtime.license_texts[key]
    for key in sorted(texts.keys()):
        text = texts[key]
        if not text.url.startswith("https://"):
            errors.append("license text '" + key + "' does not use https")
        if not _is_sha256(text.sha256):
            errors.append("license text '" + key + "' sha256 is not 64 hex characters")
    for host in sorted(EXTRACTORS.keys()):
        extractor = EXTRACTORS[host]
        if not extractor.url.startswith("https://") or not _is_sha256(extractor.sha256):
            errors.append("extractor '" + host + "' is not pinned by an https url and sha256")
    return errors

def resolve_modules(modules):
    """Returns the requested modules closed over their requirements, sorted."""
    resolved = {}
    pending = list(modules)
    for _ in range(len(MODULES) + len(modules)):
        if len(pending) == 0:
            break
        name = pending.pop()
        if name in resolved or name not in MODULES:
            continue
        resolved[name] = True
        pending.extend(MODULES[name].requires)
    return sorted(resolved.keys())

def default_request():
    """Returns the request a bare qt_sdk.configure() makes."""
    return struct(
        mirror = "",
        modules = list(DEFAULT_MODULES),
        runtime_mirror = "",
        version = QT_VERSION,
    )

def request_errors(request):
    """Returns one error string per rejected field of one acquisition request."""
    errors = []
    if request.version != QT_VERSION:
        errors.append("qt_sdk: version '" + request.version + "' is not pinned; want " + QT_VERSION)
    if len(request.modules) == 0:
        errors.append("qt_sdk: request at least one module")
    for name in request.modules:
        if name not in MODULES:
            errors.append("qt_sdk: unknown module '" + name + "'; want one of " + ", ".join(sorted(MODULES.keys())))
    for field, value in [("mirror", request.mirror), ("runtime_mirror", request.runtime_mirror)]:
        if value != "" and not value.startswith("https://"):
            errors.append("qt_sdk: " + field + " '" + value + "' must use https")
    return errors

def url_for(cell, key, mirror = ""):
    """Returns the download URL of one pinned Qt archive."""
    base = mirror if mirror != "" else QT_MIRROR
    entry = CELLS[cell]
    return base + "/" + entry.directory + "/" + entry.archives[key].archive

def runtime_url_for(cell, package, mirror = ""):
    """Returns the download URL of one pinned runtime package of one cell."""
    base = mirror if mirror != "" else RUNTIME_MIRROR
    runtime = CELLS[cell].runtime
    return base + "/" + runtime.release + "/" + package.repository + "/" + runtime.arch + "/os/Packages/" + package.file.replace("+", "%2B")

def canonical_id_for(url):
    """Returns the stable cache identity for one download URL."""
    return "dx-qt-archive:" + url

def no_match_error():
    """Returns the diagnostic for a platform with no qualified Qt cell."""
    blocked = [cell + " (" + CELLS[cell].blocker + ")" for cell in sorted(CELLS.keys()) if CELLS[cell].blocker != ""]
    return ("rules_dx: no qualified Qt " + QT_VERSION + " cell for this platform; qualified: " +
            ", ".join(qualified_cells()) + "; not qualified: " + "; ".join(blocked))

def hub_build(cells, rules):
    """Returns the hub BUILD file routing every Qt target to its platform cell."""
    lines = [
        'load("%s", "qt_rust_runtime")' % rules,
        "",
        "qt_rust_runtime(",
        '    name = "cxx_qt",',
        '    core = ":core",',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]
    for cell in sorted(cells.keys()):
        lines.append("config_setting(")
        lines.append('    name = "%s",' % cell)
        lines.append("    constraint_values = [")
        for constraint in CELLS[cell].constraints:
            lines.append('        "%s",' % constraint)
        lines.append("    ],")
        lines.append(")")
        lines.append("")
    for target in hub_targets():
        lines.append("alias(")
        lines.append('    name = "%s",' % target)
        lines.append("    actual = select({")
        for cell in sorted(cells.keys()):
            lines.append('        ":%s": "@%s//:%s",' % (cell, cells[cell], target))
        lines.append("    }, no_match_error = \"%s\")," % no_match_error())
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
        lines.append("")
    return "\n".join(lines)

def closure_libraries(cell, closure):
    """Returns the sonames of one runtime closure a Qt library of one cell links against."""
    names = list(CELLS[cell].layout.icu_libraries) if closure == "core" else []
    for package in CELLS[cell].runtime.packages:
        if package.closure == closure:
            for library in package.libraries:
                names.append(library.split("/")[-1])
        if closure == "display":
            for library in getattr(package, "display_libraries", []):
                names.append(library.split("/")[-1])
    return sorted(names)

def _unsupported(name, reason):
    return [
        "qt_unsupported(",
        '    name = "%s",' % name,
        '    reason = "%s",' % reason,
        ")",
        "",
    ]

def _library(name, spec, layout, closures):
    deps = [dep for dep in spec.deps if closures or dep not in _CLOSURE_DEPS]
    return [
        "cc_library(",
        '    name = "%s",' % name,
        '    srcs = ["lib/%s"],' % layout.library(spec.name),
        "    hdrs = glob([%s])," % ", ".join(['"include/%s/**"' % headers for headers in spec.headers]),
        "    includes = [%s]," % ", ".join(['"include"'] + ['"include/%s"' % headers for headers in spec.headers]),
        '    defines = ["%s"],' % spec.define,
        "    deps = [%s]," % ", ".join(['"%s"' % dep for dep in deps]),
        ")",
        "",
    ]

def _tool(name):
    tool = TOOLS[name]
    data = ['"%s"' % tool.path] + ['"%s"' % label for label in tool.data] + ['":runtime"']
    return [
        "cc_binary(",
        '    name = "%s",' % name,
        '    srcs = ["launcher/launcher.c", "launcher/%s.c"],' % name,
        "    data = [%s]," % ", ".join(data),
        '    features = ["-static_link_cpp_runtimes"],',
        ")",
        "",
    ]

def _closure(cell, name, closure):
    lines = [
        "cc_library(",
        '    name = "%s",' % name,
        "    srcs = [",
    ]
    for library in closure_libraries(cell, closure):
        if not library.startswith("libstdc++"):
            lines.append('        "lib/%s",' % library)
    lines.extend([
        "    ],",
        '    data = [":runtime"],',
        '    linkopts = ["-Wl,--allow-shlib-undefined"],',
        '    visibility = ["//visibility:private"],',
        ")",
        "",
    ])
    return lines

def _filegroup(name, srcs):
    return [
        "filegroup(",
        '    name = "%s",' % name,
        "    srcs = %s," % srcs,
        ")",
        "",
    ]

def cell_build(cell, modules, rules_cc, unsupported):
    """Returns the BUILD file of one acquired cell for the enabled modules."""
    entry = CELLS[cell]
    layout = entry.layout
    lines = [
        'load("%s", "cc_binary", "cc_library")' % rules_cc,
        'load("%s", "qt_unsupported")' % unsupported,
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
    ]
    lines.extend(_filegroup("runtime", 'glob(["%s"])' % layout.runtime_glob))
    lines.extend(_filegroup("licenses", 'glob(["licenses/**"])'))
    lines.extend(_filegroup("sbom", 'glob(["sbom/*.spdx.json"])'))
    if entry.runtime != None:
        lines.extend(_closure(cell, "closure", "core"))
        lines.extend(_closure(cell, "gui_closure", "gui"))
        lines.extend(_filegroup("fonts", "[%s]" % ", ".join(['"fonts/fonts.conf"'] + ['"fonts/%s"' % font.split("/")[-1] for font in entry.runtime.font.fonts])))
    else:
        lines.extend(_filegroup("fonts", "[]"))
    lines.extend(_filegroup("platforms", "[%s]" % ", ".join(['"plugins/platforms/%s"' % plugin for plugin in layout.platform_plugins])))
    for name in sorted(layout.display_plugins.keys()):
        lines.extend(_filegroup("display_" + name, "[%s]" % ", ".join(['"%s"' % plugin for plugin in layout.display_plugins[name]])))
    for name in sorted(layout.runtime_plugins.keys()):
        lines.extend(_filegroup("plugins_" + name, "[%s]" % ", ".join(['"%s"' % plugin for plugin in layout.runtime_plugins[name]])))
    enabled = {}
    for module in modules:
        for target in MODULES[module].targets:
            enabled[target] = module
    for module in sorted(MODULES.keys()):
        for target in MODULES[module].targets:
            if target in enabled:
                continue
            lines.extend(_unsupported(target, "enable module '" + module + "' in qt_sdk.configure(modules = ...)"))
    for name in sorted(_LIBRARIES.keys()):
        if _LIBRARIES[name].module in modules:
            lines.extend(_library(name, _LIBRARIES[name], layout, entry.runtime != None))
    if "metatypes" in enabled:
        lines.extend([
            "filegroup(",
            '    name = "metatypes",',
            '    srcs = glob(["metatypes/*.json"]),',
            ")",
            "",
        ])
    if "qml_imports" in enabled:
        lines.extend([
            "filegroup(",
            '    name = "qml_imports",',
            '    srcs = glob(["qml/**"]),',
            ")",
            "",
            "filegroup(",
            '    name = "qtqml_qmldir",',
            '    srcs = ["qml/QtQml/qmldir"],',
            ")",
            "",
            "filegroup(",
            '    name = "plugins",',
            '    srcs = glob(["%s"]),' % layout.plugin_glob,
            ")",
            "",
        ])
    for name in sorted(TOOLS.keys()):
        if name in enabled:
            lines.extend(_tool(name))
    return "\n".join(lines)

def _extract(ctx, extractor, archive, output, links = False):
    command = [str(ctx.path(_EXTRACT + "/7zip/" + extractor.binary)), "x", "-y", "-bd", "-o" + output]
    if links:
        command.append("-snld20")
    result = ctx.execute(command + [str(archive)])
    if result.return_code != 0:
        fail("qt_sdk: cannot extract '" + str(archive) + "': " + result.stdout + result.stderr)

def _move_real(ctx, source, destination):
    path = ctx.path(source)
    if not path.exists:
        fail("qt_sdk: '" + source + "' is missing from the pinned payload")
    if ctx.path(destination).exists:
        fail("qt_sdk: '" + destination + "' is provided twice")
    ctx.rename(path.realpath, destination)

def _acquire_rpm(ctx, extractor, package):
    root = _EXTRACT + "/rpm/" + package.name
    rpm = root + "/" + package.file
    url = runtime_url_for(ctx.attr.cell, package, ctx.attr.runtime_mirror)
    ctx.download(url = url, output = rpm, sha256 = package.sha256, canonical_id = canonical_id_for(url))
    _extract(ctx, extractor, rpm, root)
    _extract(ctx, extractor, root + "/" + package.file.removesuffix(".rpm") + ".cpio", root + "/root", links = True)
    return root + "/root"

def _acquire_runtime(ctx, extractor):
    runtime = CELLS[ctx.attr.cell].runtime
    roots = {}
    for package in runtime.packages:
        roots[package.name] = _acquire_rpm(ctx, extractor, package)
        for library in package.libraries + getattr(package, "display_libraries", []):
            _move_real(ctx, roots[package.name] + "/" + library, "lib/" + library.split("/")[-1])
    fonts = _acquire_rpm(ctx, extractor, runtime.font)
    for font in runtime.font.fonts:
        _move_real(ctx, fonts + "/" + font, "fonts/" + font.split("/")[-1])
    ctx.file("fonts/fonts.conf", _FONTS_CONF)
    ctx.rename(fonts + "/" + runtime.font.license_dir, "licenses/" + runtime.font.name)
    for package in runtime.packages:
        if package.license_from == package.name:
            ctx.rename(roots[package.name] + "/" + package.license_dir, "licenses/" + package.name)
    texts = LICENSE_TEXTS | runtime.license_texts
    for key in sorted(texts.keys()):
        ctx.download(url = texts[key].url, output = "licenses/" + key, sha256 = texts[key].sha256)
    for package in runtime.packages:
        if package.license_from != "" and package.license_from != package.name:
            for text in ctx.path("licenses/" + package.license_from).readdir():
                ctx.symlink(text, "licenses/" + package.name + "/" + text.basename)

def _check_module(ctx, name):
    path = ctx.path("modules/" + name + ".json")
    if not path.exists:
        fail("qt_sdk: the pinned archives carry no modules/" + name + ".json")
    version = json.decode(ctx.read(path)).get("version", "")
    if version != QT_VERSION:
        fail("qt_sdk: modules/" + name + ".json is version '" + version + "', want " + QT_VERSION)

def _qt_cell_repo_impl(ctx):
    errors = pin_errors()
    if len(errors) > 0:
        fail("qt_sdk: " + "; ".join(errors))
    host = host_key(ctx.os.name, ctx.os.arch)
    if host not in EXTRACTORS:
        fail("qt_sdk: the pinned extractor runs on " + ", ".join(sorted(EXTRACTORS.keys())) + " hosts only, not " + ctx.os.name + " " + ctx.os.arch)
    extractor = EXTRACTORS[host]
    entry = CELLS[ctx.attr.cell]
    layout = entry.layout
    ctx.download_and_extract(
        url = extractor.url,
        output = _EXTRACT + "/7zip",
        sha256 = extractor.sha256,
        canonical_id = canonical_id_for(extractor.url),
    )
    for key in _archive_keys(entry) + ctx.attr.modules:
        url = url_for(ctx.attr.cell, key, ctx.attr.mirror)
        archive = _EXTRACT + "/" + entry.archives[key].archive
        ctx.download(url = url, output = archive, sha256 = entry.archives[key].sha256, canonical_id = canonical_id_for(url))
        kept = MODULES[key].files if key in MODULES else []
        _extract(ctx, extractor, archive, _EXTRACT + "/" + key if key == "icu" or len(kept) > 0 else ".", links = True)
        for path in kept:
            _move_real(ctx, _EXTRACT + "/" + key + "/" + path, path)
    for library in layout.icu_libraries:
        _move_real(ctx, _EXTRACT + "/icu/" + library, "lib/" + library)
    if entry.runtime != None:
        _acquire_runtime(ctx, extractor)
    _check_module(ctx, "Core")
    for module in ctx.attr.modules:
        if not ctx.path("sbom/" + module + "-" + QT_VERSION + ".spdx.json").exists:
            fail("qt_sdk: the pinned archives carry no sbom/" + module + "-" + QT_VERSION + ".spdx.json")
    if "qtdeclarative" in ctx.attr.modules:
        _check_module(ctx, "Qml")
    if "qttools" in ctx.attr.modules:
        _check_module(ctx, "Linguist")
    ctx.delete(_EXTRACT)
    ctx.file("launcher/launcher.c", ctx.read(getattr(ctx.attr, _LAUNCHERS[layout.launcher])))
    for name in sorted(TOOLS.keys()):
        ctx.file("launcher/" + name + ".c", _LAUNCH % (ctx.name, TOOLS[name].path))
    ctx.file("BUILD.bazel", cell_build(
        ctx.attr.cell,
        ctx.attr.modules,
        str(Label("@rules_cc//cc:defs.bzl")),
        str(Label("//qt/sdk:unsupported.bzl")),
    ))
    return ctx.repo_metadata(reproducible = True)

def _qt_hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build(json.decode(ctx.attr.cells), str(Label("//qt/rules:defs.bzl"))))
    return ctx.repo_metadata(reproducible = True)

_qt_cell_repo = repository_rule(
    implementation = _qt_cell_repo_impl,
    attrs = {
        "cell": attr.string(mandatory = True),
        "mirror": attr.string(default = ""),
        "modules": attr.string_list(mandatory = True),
        "runtime_mirror": attr.string(default = ""),
        "_launcher": attr.label(default = "//qt/sdk:launcher.c"),
    },
)

_qt_hub_repo = repository_rule(
    implementation = _qt_hub_repo_impl,
    attrs = {
        "cells": attr.string(mandatory = True),
    },
)

def _qt_sdk_impl(module_ctx):
    errors = pin_errors()
    if len(errors) > 0:
        fail("qt_sdk: " + "; ".join(errors))
    requests = []
    for module in module_ctx.modules:
        for tag in module.tags.configure:
            requests.append(struct(
                mirror = tag.mirror,
                modules = list(tag.modules),
                runtime_mirror = tag.runtime_mirror,
                version = tag.version,
            ))
    if len(requests) != 1:
        fail("qt_sdk: call qt_sdk.configure() exactly once")
    request = requests[0]
    failures = request_errors(request)
    if len(failures) > 0:
        fail("; ".join(failures))
    cells = {}
    for cell in qualified_cells():
        cells[cell] = cell_repository_name(cell)
        _qt_cell_repo(
            name = cells[cell],
            cell = cell,
            mirror = request.mirror,
            modules = resolve_modules(request.modules),
            runtime_mirror = request.runtime_mirror,
        )
    _qt_hub_repo(name = HUB_NAME, cells = json.encode(cells))

qt_sdk = module_extension(
    implementation = _qt_sdk_impl,
    doc = "Acquires the pinned Qt SDK and runtime closure on first use.",
    tag_classes = {
        "configure": tag_class(
            doc = "Requests Qt modules and optional mirrors.",
            attrs = {
                "mirror": attr.string(default = "", doc = "Https mirror of the Qt online repository."),
                "modules": attr.string_list(default = DEFAULT_MODULES, doc = "Qt modules to acquire."),
                "runtime_mirror": attr.string(default = "", doc = "Https mirror of vault.almalinux.org."),
                "version": attr.string(default = QT_VERSION, doc = "Qt version; only the pinned one is accepted."),
            },
        ),
    },
)
