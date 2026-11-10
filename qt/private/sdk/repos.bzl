"""Opt-in hermetic Qt SDK acquisition."""

load("//cc/private/toolchains:pins.bzl", "VC_REDIST_VERSION")
load("//core:host.bzl", "host_cell_of")
load("//core:platform_support.bzl", "cells_support")
load("//core:seven_zip.bzl", "seven_zip_fetch")
load(
    ":pins.bzl",
    "CELLS",
    "CLOSURES",
    "DEFAULT_MODULES",
    "EXTRACTORS",
    "LAYOUTS",
    "LICENSE_TEXTS",
    "MODULES",
    "QT_MIRROR",
    "QT_VERSION",
    "RUNTIME_MIRROR",
    "RUNTIME_REPOSITORIES",
)

HUB_NAME = "dx_qt"

VCRUNTIME_FILES = ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll", "msvcp140_1.dll", "msvcp140_2.dll"]

SHARED_TARGETS = ["licenses", "needs", "runtime", "sbom"]

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

_TOKEN = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_NAME = "abcdefghijklmnopqrstuvwxyz0123456789_"

_KINDS = {
    "list": "a list",
    "object": "an object",
    "runtime": "an object or null",
    "string": "a string",
    "strings": "a list of strings",
    "table": "an object of string lists",
}

_SPEC_FIELDS = {"archives": "object", "directory": "string", "release": "string", "runtime": "runtime"}

_SPEC_OPTIONAL = {"needs": "object"}

_NEEDS_FIELDS = {"library": "table", "plugin": "table", "runtime_plugin": "table"}

_ARCHIVE_FIELDS = {"archive": "string", "sha256": "string"}

_RUNTIME_FIELDS = {"arch": "string", "font": "object", "license_texts": "object", "packages": "list", "release": "string"}

_RUNTIME_OPTIONAL = {"glibc": "string", "libstdcxx": "string"}

_FONT_FIELDS = {
    "file": "string",
    "fonts": "strings",
    "license": "string",
    "license_dir": "string",
    "name": "string",
    "repository": "string",
    "sha256": "string",
}

_PACKAGE_FIELDS = {
    "closure": "string",
    "file": "string",
    "libraries": "strings",
    "license": "string",
    "license_dir": "string",
    "license_from": "string",
    "name": "string",
    "repository": "string",
    "sha256": "string",
}

_PACKAGE_OPTIONAL = {"display_libraries": "strings"}

_TEXT_FIELDS = {"sha256": "string", "url": "string"}

_QML = [":qml", ":network", ":core"]

_CLOSURE_DEPS = [":closure", ":gui_closure"]

_LAUNCHERS = {"exec": "", "posix": "_launcher"}

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

_WINDOWS_FONT = struct(
    sha256 = "fa9ca4d13871dd122f61258a80d01751d603b4d3ee14095d65453b4e846e17d7",
    url = "https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.tar.bz2",
)

_LAUNCH = """int dx_qt_launch(int argc, char **argv, const char *repository, const char *tool);

int main(int argc, char **argv) {
  return dx_qt_launch(argc, argv, "%s", "%s");
}
"""

def cell_repository_name(cell):
    """Returns the repository name one qualified cell is acquired into."""
    return _CELL_PREFIX + cell

def qualified_cells(cells = CELLS):
    """Returns the cells with pinned archives and probe evidence."""
    return [cell for cell in sorted(cells.keys()) if cells[cell].blocker == ""]

PLATFORM_SUPPORT = cells_support("//qt:defs.bzl", "QT_COMPATIBLE_WITH", CELLS, qualified_cells(), "//qt/private", "constraints", settings = {cell: Label("//qt/private/platforms:cell_" + cell) for cell in CELLS})

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

def _is_token(value, extra = ""):
    if value == "":
        return False
    for index in range(len(value)):
        if value[index] not in _TOKEN + extra:
            return False
    return True

def _is_file_name(value):
    if value in ["", ".", ".."]:
        return False
    for bad in ["/", "\\", "?", "#"]:
        if bad in value:
            return False
    return True

def _is_relative(value, banned = ""):
    if value == "" or value.startswith("/") or "\\" in value:
        return False
    for bad in banned.elems():
        if bad in value:
            return False
    for segment in value.split("/"):
        if segment in ["", ".", ".."]:
            return False
    return True

def _archive_errors(cell, key, archive, release):
    errors = []
    if not _is_file_name(archive.archive):
        errors.append("cell '" + cell + "' archive '" + key + "' file '" + archive.archive + "' is not a plain file name")
    if not archive.archive.startswith(release):
        errors.append("cell '" + cell + "' archive '" + key + "' is not from release " + release)
    if not archive.archive.endswith(".7z"):
        errors.append("cell '" + cell + "' archive '" + key + "' is not a .7z archive")
    if not _is_sha256(archive.sha256):
        errors.append("cell '" + cell + "' archive '" + key + "' sha256 is not 64 hex characters")
    return errors

def host_key(os, arch):
    """Returns the EXTRACTORS key of one host from Bazel's os name and arch."""
    return host_cell_of(os, arch)

def cell_errors(cell, entry):
    """Returns one error string per pin of one cell entry that is not ready to acquire."""
    errors = []
    if len(entry.constraints) == 0:
        errors.append("cell '" + cell + "' names no platform constraints")
    if entry.blocker != "":
        if len(entry.archives) == 0:
            if entry.directory != "" or entry.release != "" or entry.runtime != None:
                errors.append("cell '" + cell + "' is blocked but names a release")
            return errors
    if entry.directory == "":
        errors.append("cell '" + cell + "' names no repository directory")
    elif not _is_relative(entry.directory, "?#:"):
        errors.append("cell '" + cell + "' directory '" + entry.directory + "' is not a relative url path")
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
    if not _is_token(package.name, "+"):
        errors.append("runtime package '" + package.name + "' name is not a plain package name")
    if not _is_file_name(package.file):
        errors.append("runtime package '" + package.name + "' file '" + package.file + "' is not a plain file name")
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

def runtime_errors(cell, runtime):
    """Returns one error string per pin of one runtime closure that is not ready to acquire."""
    errors = []
    for field, value in [("release", runtime.release), ("arch", runtime.arch)]:
        if not _is_token(value):
            errors.append("cell '" + cell + "' runtime " + field + " '" + value + "' is not made of letters, digits, '.', '_' and '-'")
    names = [package.name for package in runtime.packages]
    if names != sorted(names):
        errors.append("cell '" + cell + "' runtime packages are not sorted by name")
    for package in runtime.packages:
        errors.extend(_package_errors(runtime, package, names))
    errors.extend(_rpm_errors(runtime.font, "noarch"))
    if len(runtime.font.fonts) == 0 or runtime.font.license_dir == "":
        errors.append("font package '" + runtime.font.name + "' carries no fonts or license directory")
    for package in runtime.packages + [runtime.font]:
        paths = getattr(package, "libraries", []) + getattr(package, "display_libraries", []) + getattr(package, "fonts", [])
        if package.license_dir != "":
            paths = paths + [package.license_dir]
        for path in paths:
            if not _is_relative(path):
                errors.append("runtime package '" + package.name + "' path '" + path + "' is not a relative path")
    for key in sorted(runtime.license_texts.keys()):
        if not _is_relative(key):
            errors.append("cell '" + cell + "' license text '" + key + "' is not a relative path")
    return errors

def pin_errors(cells = CELLS):
    """Returns one error string per pin that is not ready to acquire."""
    errors = []
    if len(qualified_cells(cells)) == 0:
        errors.append("no cell is qualified")
    for cell in sorted(cells.keys()):
        errors.extend(cell_errors(cell, cells[cell]))
    for name in sorted(MODULES.keys()):
        for required in MODULES[name].requires:
            if required not in MODULES:
                errors.append("module '" + name + "' requires unknown module '" + required + "'")
    texts = dict(LICENSE_TEXTS)
    for cell in qualified_cells(cells):
        runtime = cells[cell].runtime
        if runtime == None:
            continue
        errors.extend(runtime_errors(cell, runtime))
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

def cell_spec(entry):
    """Returns the JSON spec of one cell entry, without its layout, constraints and blocker."""
    spec = {
        "archives": entry.archives,
        "directory": entry.directory,
        "release": entry.release,
        "runtime": entry.runtime,
    }
    if entry.needs != None:
        spec["needs"] = entry.needs
    return spec

def _strings(value):
    return type(value) == "list" and all([type(item) == "string" for item in value])

def _kind_ok(value, kind):
    if kind == "strings":
        return _strings(value)
    if kind == "object":
        return type(value) == "dict"
    if kind == "table":
        return type(value) == "dict" and all([_strings(item) for item in value.values()])
    if kind == "runtime":
        return value == None or type(value) == "dict"
    return type(value) == kind

def _decode(where, value, required, optional, errors):
    if type(value) != "dict":
        errors.append(where + " is not an object")
        return None
    before = len(errors)
    fields = {}
    for key in sorted(value.keys()):
        kind = required.get(key, optional.get(key))
        if kind == None:
            errors.append(where + " has unknown key '" + key + "'")
        elif not _kind_ok(value[key], kind):
            errors.append(where + " '" + key + "' is not " + _KINDS[kind])
        else:
            fields[key] = value[key]
    for key in sorted(required.keys()):
        if key not in value:
            errors.append(where + " is missing '" + key + "'")
    return fields if len(errors) == before else None

def _runtime_from_json(where, value, errors):
    fields = _decode(where, value, _RUNTIME_FIELDS, _RUNTIME_OPTIONAL, errors)
    if fields == None:
        return None
    before = len(errors)
    font = _decode(where + ".font", fields["font"], _FONT_FIELDS, {}, errors)
    packages = []
    for index, package in enumerate(fields["packages"]):
        decoded = _decode(where + ".packages[" + str(index) + "]", package, _PACKAGE_FIELDS, _PACKAGE_OPTIONAL, errors)
        if decoded != None:
            packages.append(struct(**decoded))
    texts = {}
    for key in sorted(fields["license_texts"].keys()):
        text = _decode(where + ".license_texts." + key, fields["license_texts"][key], _TEXT_FIELDS, {}, errors)
        if text != None:
            texts[key] = struct(**text)
    if len(errors) > before:
        return None
    fields["font"] = struct(**font)
    fields["license_texts"] = texts
    fields["packages"] = packages
    return struct(**fields)

def cell_from_json(name, data, constraints, layout):
    """Returns one cell entry decoded from its JSON spec, and one error string per rejected field."""
    errors = []
    where = "cell '" + name + "' spec"
    spec = _decode(where, data, _SPEC_FIELDS, _SPEC_OPTIONAL, errors)
    if spec == None:
        return None, errors
    archives = {}
    for key in sorted(spec["archives"].keys()):
        archive = _decode(where + " archives." + key, spec["archives"][key], _ARCHIVE_FIELDS, {}, errors)
        if archive != None:
            archives[key] = struct(**archive)
    runtime = None
    if spec["runtime"] != None:
        runtime = _runtime_from_json(where + " runtime", spec["runtime"], errors)
    needs = None
    if "needs" in spec:
        decoded = _decode(where + " needs", spec["needs"], _NEEDS_FIELDS, {}, errors)
        if decoded != None:
            needs = struct(**decoded)
    if len(errors) > 0:
        return None, errors
    return struct(
        archives = archives,
        blocker = "",
        constraints = list(constraints),
        directory = spec["directory"],
        layout = layout,
        needs = needs,
        release = spec["release"],
        runtime = runtime,
    ), []

def effective_cells(consumer):
    """Returns the pinned cells overlaid by the consumer cells of the same name."""
    cells = dict(CELLS)
    cells.update(consumer)
    return cells

def canonical_constraints(constraints):
    """Returns the canonical, sorted labels of one constraint list."""
    return sorted([str(Label(constraint)) for constraint in constraints])

def _is_cell_name(value):
    if value == "":
        return False
    for index in range(len(value)):
        if value[index] not in _NAME:
            return False
    return True

def consumer_cells(tags):
    """Returns the consumer cells decoded from qt_sdk.cell tags, and one error string per rejected tag."""
    cells = {}
    errors = []
    seen = {}
    reserved = hub_targets() + ["cxx_qt"]
    for tag in tags:
        where = "cell '" + tag.name + "'"
        before = len(errors)
        if not _is_cell_name(tag.name):
            errors.append(where + " name is not made of lowercase letters, digits and '_'")
        elif tag.name in reserved:
            errors.append(where + " name is taken by a @" + HUB_NAME + " target")
        if tag.name in seen:
            errors.append(where + " is declared twice")
        seen[tag.name] = True
        if tag.layout not in LAYOUTS:
            errors.append(where + " layout '" + tag.layout + "' is not one of " + ", ".join(sorted(LAYOUTS.keys())))
        data = json.decode(tag.spec, None)
        if type(data) != "dict":
            errors.append(where + " spec is not a JSON object")
        if len(errors) > before:
            continue
        entry, failures = cell_from_json(tag.name, data, tag.constraints, LAYOUTS[tag.layout])
        errors.extend(failures)
        if entry != None:
            cells[tag.name] = entry
    table = effective_cells(cells)
    for name in sorted(cells.keys()):
        mine = canonical_constraints(cells[name].constraints)
        for other in qualified_cells(table):
            if other != name and canonical_constraints(table[other].constraints) == mine:
                errors.append("cell '" + name + "' constraints match cell '" + other + "'; add a constraint that sets it apart")
    return cells, errors

def url_for(cell, key, mirror = "", cells = CELLS):
    """Returns the download URL of one pinned Qt archive."""
    base = mirror if mirror != "" else QT_MIRROR
    entry = cells[cell]
    return base + "/" + entry.directory + "/" + entry.archives[key].archive

def runtime_url_for(cell, package, mirror = "", cells = CELLS):
    """Returns the download URL of one pinned runtime package of one cell."""
    base = mirror if mirror != "" else RUNTIME_MIRROR
    runtime = cells[cell].runtime
    return base + "/" + runtime.release + "/" + package.repository + "/" + runtime.arch + "/os/Packages/" + package.file.replace("+", "%2B")

def canonical_id_for(url):
    """Returns the stable cache identity for one download URL."""
    return "dx-qt-archive:" + url

def no_match_error(cells = CELLS):
    """Returns the diagnostic for a platform with no qualified Qt cell."""
    blocked = [cell + " (" + cells[cell].blocker + ")" for cell in sorted(cells.keys()) if cells[cell].blocker != ""]
    return ("rules_dx: no qualified Qt " + QT_VERSION + " cell for this platform; qualified: " +
            ", ".join(qualified_cells(cells)) + "; not qualified: " + "; ".join(blocked))

def hub_build(cells, rules, constraints = None, no_match = None):
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
    if constraints == None:
        constraints = {cell: CELLS[cell].constraints for cell in cells}
    if no_match == None:
        no_match = no_match_error()
    for cell in sorted(cells.keys()):
        lines.append("config_setting(")
        lines.append('    name = "%s",' % cell)
        lines.append("    constraint_values = [")
        for constraint in constraints[cell]:
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
        lines.append("    }, no_match_error = \"%s\")," % no_match)
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
        lines.append("")
    return "\n".join(lines)

def closure_libraries(cell, closure, cells = CELLS):
    """Returns the sonames of one runtime closure a Qt library of one cell links against."""
    names = list(cells[cell].layout.icu_libraries) if closure == "core" else []
    for package in cells[cell].runtime.packages:
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
    library = "lib/" + layout.library(spec.name)
    lines = []
    if layout.link == "import":
        lines.extend([
            "cc_import(",
            '    name = "%s_import",' % name,
            '    interface_library = "%s",' % library,
            '    shared_library = "%s",' % layout.shared(spec.name),
            ")",
            "",
        ])
        link = []
        deps = [":" + name + "_import"] + deps
    elif layout.link == "framework":
        link = [
            '    additional_linker_inputs = ["%s"],' % library,
            '    linkopts = ["$(execpath %s)"],' % library,
        ]
    else:
        link = ['    srcs = ["%s"],' % library]
    return lines + [
        "cc_library(",
        '    name = "%s",' % name,
    ] + link + [
        "    hdrs = glob([%s])," % ", ".join(['"include/%s/**"' % headers for headers in spec.headers]),
        "    includes = [%s]," % ", ".join(['"include"'] + ['"include/%s"' % headers for headers in spec.headers]),
        '    defines = ["%s"],' % spec.define,
        "    deps = [%s]," % ", ".join(['"%s"' % dep for dep in deps]),
        ")",
        "",
    ]

def _tool(name, layout):
    tool = TOOLS[name]
    path = layout.tool(tool.path)
    if layout.launcher == "exec":
        return [
            "qt_tool(",
            '    name = "%s",' % name,
            '    program = "%s",' % path,
            "    data = [%s]," % ", ".join(['"%s"' % label for label in tool.data] + ['":runtime"']),
            ")",
            "",
        ]
    data = ['"%s"' % path] + ['"%s"' % label for label in tool.data] + ['":runtime"']
    return [
        "cc_binary(",
        '    name = "%s",' % name,
        '    srcs = ["launcher/launcher.c", "launcher/%s.c"],' % name,
        "    data = [%s]," % ", ".join(data),
        '    features = ["-static_link_cpp_runtimes"],',
        ")",
        "",
    ]

def _closure(cell, name, closure, cells):
    lines = [
        "cc_library(",
        '    name = "%s",' % name,
        "    srcs = [",
    ]
    for library in closure_libraries(cell, closure, cells):
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

def _needs(cell, needs):
    if needs == None:
        return _unsupported("needs", "cell '" + cell + "' spec carries no needs tables; qt_bundle cannot package it")
    return [
        "qt_needs(",
        '    name = "needs",',
        "    library = %r," % needs.library,
        "    plugin = %r," % needs.plugin,
        "    runtime_plugin = %r," % needs.runtime_plugin,
        ")",
        "",
    ]

def cell_build(cell, modules, rules_cc, unsupported, needs, cells = CELLS, tool = ""):
    """Returns the BUILD file of one acquired cell for the enabled modules."""
    entry = cells[cell]
    layout = entry.layout
    lines = [
        'load("%s", "cc_binary", "cc_import", "cc_library")' % rules_cc,
        'load("%s", "qt_needs")' % needs,
        'load("%s", "qt_unsupported")' % unsupported,
        "",
    ]
    if layout.launcher == "exec":
        lines.insert(3, 'load("%s", "qt_tool")' % tool)
    lines += [
        'package(default_visibility = ["//visibility:public"])',
        "",
    ]
    lines.extend(_needs(cell, entry.needs))
    lines.extend(_filegroup("runtime", 'glob(["%s"])' % layout.runtime_glob))
    lines.extend(_filegroup("licenses", 'glob(["licenses/**"])'))
    lines.extend(_filegroup("sbom", 'glob(["sbom/*.spdx.json"])'))
    if entry.runtime != None:
        lines.extend(_closure(cell, "closure", "core", cells))
        lines.extend(_closure(cell, "gui_closure", "gui", cells))
        lines.extend(_filegroup("fonts", "[%s]" % ", ".join(['"fonts/fonts.conf"'] + ['"fonts/%s"' % font.split("/")[-1] for font in entry.runtime.font.fonts])))
    elif layout.os == "windows":
        lines.extend(_filegroup("fonts", '["fonts/DejaVuSans.ttf"]'))
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
            lines.extend(_tool(name, layout))
    defined = {}
    for line in lines:
        if line.startswith('    name = "'):
            defined[line.split('"')[1]] = True
    for name in hub_targets():
        if name not in defined:
            lines.extend(_unsupported(name, "not available on the " + cell + " Qt cell"))
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

def _acquire_rpm(ctx, extractor, package, cells):
    root = _EXTRACT + "/rpm/" + package.name
    rpm = root + "/" + package.file
    url = runtime_url_for(ctx.attr.cell, package, ctx.attr.runtime_mirror, cells)
    ctx.download(url = url, output = rpm, sha256 = package.sha256, canonical_id = canonical_id_for(url))
    _extract(ctx, extractor, rpm, root)
    _extract(ctx, extractor, root + "/" + package.file.removesuffix(".rpm") + ".cpio", root + "/root", links = True)
    return root + "/root"

def _acquire_runtime(ctx, extractor, cells):
    runtime = cells[ctx.attr.cell].runtime
    roots = {}
    for package in runtime.packages:
        roots[package.name] = _acquire_rpm(ctx, extractor, package, cells)
        for library in package.libraries + getattr(package, "display_libraries", []):
            _move_real(ctx, roots[package.name] + "/" + library, "lib/" + library.split("/")[-1])
    fonts = _acquire_rpm(ctx, extractor, runtime.font, cells)
    for font in runtime.font.fonts:
        _move_real(ctx, fonts + "/" + font, "fonts/" + font.split("/")[-1])
    ctx.file("fonts/fonts.conf", _FONTS_CONF)
    ctx.rename(fonts + "/" + runtime.font.license_dir, "licenses/" + runtime.font.name)
    for package in runtime.packages:
        if package.license_from == package.name:
            ctx.rename(roots[package.name] + "/" + package.license_dir, "licenses/" + package.name)
    for key in sorted(runtime.license_texts.keys()):
        text = runtime.license_texts[key]
        ctx.download(url = text.url, output = "licenses/" + key, sha256 = text.sha256)
    for package in runtime.packages:
        if package.license_from != "" and package.license_from != package.name:
            for text in ctx.path("licenses/" + package.license_from).readdir():
                ctx.symlink(text, "licenses/" + package.name + "/" + text.basename)

def _place_vcruntime(ctx):
    arch = "arm64" if "arm64" in ctx.attr.cell else "x64"
    redist = ctx.path(Label("@msvc_runtime//:BUILD.bazel")).dirname.get_child("sysroot/Contents/VC/Redist/MSVC/{}/{}".format(VC_REDIST_VERSION, arch))
    if not redist.exists:
        return
    for directory in redist.readdir():
        for dll in directory.readdir():
            if dll.basename.lower() in VCRUNTIME_FILES:
                ctx.symlink(dll, "bin/" + dll.basename)

def _normalize_frameworks(ctx):
    for framework in ctx.path("lib").readdir():
        if not framework.basename.endswith(".framework"):
            continue
        name = framework.basename.removesuffix(".framework")
        _move_real(ctx, "lib/" + framework.basename + "/Versions/A/Headers", "include/" + name)
        for link in ["Headers", "Resources", name, "Versions/Current"]:
            ctx.delete("lib/" + framework.basename + "/" + link)

def _check_module(ctx, name):
    path = ctx.path("modules/" + name + ".json")
    if not path.exists:
        fail("qt_sdk: the pinned archives carry no modules/" + name + ".json")
    version = json.decode(ctx.read(path)).get("version", "")
    if version != QT_VERSION:
        fail("qt_sdk: modules/" + name + ".json is version '" + version + "', want " + QT_VERSION)

def _cell_table(ctx):
    if ctx.attr.entry == "":
        return CELLS
    data = json.decode(ctx.attr.entry)
    entry, errors = cell_from_json(ctx.attr.cell, data["spec"], data["constraints"], LAYOUTS[data["layout"]])
    if len(errors) > 0:
        fail("qt_sdk: " + "; ".join(errors))
    return effective_cells({ctx.attr.cell: entry})

def _place_font(ctx):
    ctx.download_and_extract(
        url = _WINDOWS_FONT.url,
        output = _EXTRACT + "/dejavu",
        sha256 = _WINDOWS_FONT.sha256,
        canonical_id = canonical_id_for(_WINDOWS_FONT.url),
    )
    root = _EXTRACT + "/dejavu/dejavu-fonts-ttf-2.37"
    _move_real(ctx, root + "/ttf/DejaVuSans.ttf", "fonts/DejaVuSans.ttf")
    _move_real(ctx, root + "/LICENSE", "licenses/dejavu-sans-fonts/LICENSE")

def _qt_cell_repo_impl(ctx):
    cells = _cell_table(ctx)
    errors = pin_errors(cells)
    if len(errors) > 0:
        fail("qt_sdk: " + "; ".join(errors))
    host = host_key(ctx.os.name, ctx.os.arch)
    if host not in EXTRACTORS:
        fail("qt_sdk: the pinned extractor runs on " + ", ".join(sorted(EXTRACTORS.keys())) + " hosts only, not " + ctx.os.name + " " + ctx.os.arch)
    extractor = EXTRACTORS[host]
    entry = cells[ctx.attr.cell]
    layout = entry.layout
    seven_zip_fetch(ctx, _EXTRACT + "/7zip")
    for key in _archive_keys(entry) + ctx.attr.modules:
        url = url_for(ctx.attr.cell, key, ctx.attr.mirror, cells)
        archive = _EXTRACT + "/" + entry.archives[key].archive
        ctx.download(url = url, output = archive, sha256 = entry.archives[key].sha256, canonical_id = canonical_id_for(url))
        kept = [layout.tool(path) if path.startswith("bin/") else path for path in MODULES[key].files] if key in MODULES else []
        _extract(ctx, extractor, archive, _EXTRACT + "/" + key if key == "icu" or len(kept) > 0 else ".", links = True)
        for path in kept:
            _move_real(ctx, _EXTRACT + "/" + key + "/" + path, path)
    for library in layout.icu_libraries:
        _move_real(ctx, _EXTRACT + "/icu/" + library, "lib/" + library)
    for key in sorted(LICENSE_TEXTS.keys()):
        if key.startswith("icu/") and len(layout.icu_libraries) == 0:
            continue
        ctx.download(url = LICENSE_TEXTS[key].url, output = "licenses/" + key, sha256 = LICENSE_TEXTS[key].sha256)
    if entry.runtime != None:
        _acquire_runtime(ctx, extractor, cells)
    if layout.os == "windows":
        _place_font(ctx)
        _place_vcruntime(ctx)
        ctx.file("licenses/msvc-redistributable/NOTICE.txt", ctx.read(Label("//cc/private/toolchains:notices/MSVC_REDISTRIBUTABLE.txt")))
    if layout.link == "framework":
        _normalize_frameworks(ctx)
    _check_module(ctx, "Core")
    for module in ctx.attr.modules:
        if not ctx.path("sbom/" + module + "-" + QT_VERSION + ".spdx.json").exists:
            fail("qt_sdk: the pinned archives carry no sbom/" + module + "-" + QT_VERSION + ".spdx.json")
    if "qtdeclarative" in ctx.attr.modules:
        _check_module(ctx, "Qml")
    if "qttools" in ctx.attr.modules:
        _check_module(ctx, "Linguist")
    ctx.delete(_EXTRACT)
    if layout.launcher == "posix":
        ctx.file("launcher/launcher.c", ctx.read(getattr(ctx.attr, _LAUNCHERS[layout.launcher])))
        for name in sorted(TOOLS.keys()):
            ctx.file("launcher/" + name + ".c", _LAUNCH % (ctx.name, TOOLS[name].path))
    ctx.file("BUILD.bazel", cell_build(
        ctx.attr.cell,
        ctx.attr.modules,
        str(Label("@rules_cc//cc:defs.bzl")),
        str(Label("//qt/private/sdk:unsupported.bzl")),
        str(Label("//qt/private/sdk:needs.bzl")),
        cells,
        str(Label("//qt/private/sdk:tool.bzl")),
    ))
    return ctx.repo_metadata(reproducible = True)

qt_cell_repo = repository_rule(
    implementation = _qt_cell_repo_impl,
    attrs = {
        "cell": attr.string(
            doc = "Qt platform cell whose pinned archives are acquired.",
            mandatory = True,
        ),
        "entry": attr.string(
            doc = "JSON of a consumer cell spec, constraints and layout; empty for a pinned cell.",
            default = "",
        ),
        "mirror": attr.string(
            doc = "Optional https mirror of the Qt online repository.",
            default = "",
        ),
        "modules": attr.string_list(
            doc = "Qt modules to acquire, closed over their requirements.",
            mandatory = True,
        ),
        "runtime_mirror": attr.string(
            doc = "Optional https mirror for the Qt runtime packages.",
            default = "",
        ),
        "_launcher": attr.label(default = "//qt/private/sdk:launcher.c"),
    },
)
