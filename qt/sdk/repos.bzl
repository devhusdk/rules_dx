"""Opt-in hermetic Qt SDK acquisition."""

load(
    ":pins.bzl",
    "CELLS",
    "DEFAULT_MODULES",
    "EXTRACTOR",
    "ICU_LIBRARIES",
    "LICENSE_TEXTS",
    "MODULES",
    "QT_MIRROR",
    "QT_RELEASE",
    "QT_VERSION",
    "RUNTIME_MIRROR",
    "RUNTIME_PACKAGES",
    "UNSUPPORTED_TARGETS",
)

HUB_NAME = "dx_qt"

SHARED_TARGETS = ["licenses", "runtime", "sbom"]

TOOLS = {
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

_LIBRARIES = {
    "core": struct(deps = [":closure"], headers = ["QtCore"], library = "libQt6Core.so.6"),
    "network": struct(deps = [":core"], headers = ["QtNetwork"], library = "libQt6Network.so.6"),
    "qml": struct(deps = [":network", ":core"], headers = ["QtQml", "QtQmlIntegration"], library = "libQt6Qml.so.6"),
    "test": struct(deps = [":core"], headers = ["QtTest"], library = "libQt6Test.so.6"),
}

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
    names = list(SHARED_TARGETS) + list(UNSUPPORTED_TARGETS.keys())
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

def _archive_errors(cell, key, archive):
    errors = []
    if not archive.archive.startswith(QT_RELEASE):
        errors.append("cell '" + cell + "' archive '" + key + "' is not from release " + QT_RELEASE)
    if not archive.archive.endswith(".7z"):
        errors.append("cell '" + cell + "' archive '" + key + "' is not a .7z archive")
    if not _is_sha256(archive.sha256):
        errors.append("cell '" + cell + "' archive '" + key + "' sha256 is not 64 hex characters")
    return errors

def _cell_errors(cell):
    entry = CELLS[cell]
    errors = []
    if len(entry.constraints) == 0:
        errors.append("cell '" + cell + "' names no platform constraints")
    if entry.blocker != "":
        if len(entry.archives) > 0 or entry.directory != "":
            errors.append("cell '" + cell + "' is blocked but names archives")
        return errors
    if entry.directory == "":
        errors.append("cell '" + cell + "' names no repository directory")
    for key in ["icu"] + sorted(MODULES.keys()):
        if key not in entry.archives:
            errors.append("cell '" + cell + "' pins no '" + key + "' archive")
            continue
        errors.extend(_archive_errors(cell, key, entry.archives[key]))
    return errors

def _package_errors(package, names):
    errors = []
    if not package.file.startswith(package.name + "-") or not package.file.endswith(".x86_64.rpm"):
        errors.append("runtime package '" + package.name + "' file '" + package.file + "' is not its x86_64 rpm")
    if not _is_sha256(package.sha256):
        errors.append("runtime package '" + package.name + "' sha256 is not 64 hex characters")
    if len(package.libraries) == 0:
        errors.append("runtime package '" + package.name + "' carries no libraries")
    if package.license == "":
        errors.append("runtime package '" + package.name + "' names no license")
    if package.license_from == "":
        texts = [key for key in LICENSE_TEXTS.keys() if key.startswith(package.name + "/")]
        if package.license_dir != "" or len(texts) == 0:
            errors.append("runtime package '" + package.name + "' has no license text")
    elif package.license_from not in names or package.license_dir == "":
        errors.append("runtime package '" + package.name + "' takes its license from an unknown package")
    return errors

def pin_errors():
    """Returns one error string per pin that is not ready to acquire."""
    errors = []
    if len(qualified_cells()) == 0:
        errors.append("no cell is qualified")
    for cell in sorted(CELLS.keys()):
        errors.extend(_cell_errors(cell))
    for name in sorted(MODULES.keys()):
        for required in MODULES[name].requires:
            if required not in MODULES:
                errors.append("module '" + name + "' requires unknown module '" + required + "'")
    names = [package.name for package in RUNTIME_PACKAGES]
    if names != sorted(names):
        errors.append("runtime packages are not sorted by name")
    for package in RUNTIME_PACKAGES:
        errors.extend(_package_errors(package, names))
    for key in sorted(LICENSE_TEXTS.keys()):
        text = LICENSE_TEXTS[key]
        if not text.url.startswith("https://"):
            errors.append("license text '" + key + "' does not use https")
        if not _is_sha256(text.sha256):
            errors.append("license text '" + key + "' sha256 is not 64 hex characters")
    if not EXTRACTOR.url.startswith("https://") or not _is_sha256(EXTRACTOR.sha256):
        errors.append("extractor is not pinned by an https url and sha256")
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

def runtime_url_for(package, mirror = ""):
    """Returns the download URL of one pinned runtime package."""
    base = mirror if mirror != "" else RUNTIME_MIRROR
    return base + "/" + package.file.replace("+", "%2B")

def canonical_id_for(url):
    """Returns the stable cache identity for one download URL."""
    return "dx-qt-archive:" + url

def no_match_error():
    """Returns the diagnostic for a platform with no qualified Qt cell."""
    blocked = [cell + " (" + CELLS[cell].blocker + ")" for cell in sorted(CELLS.keys()) if CELLS[cell].blocker != ""]
    return ("rules_dx: no qualified Qt " + QT_VERSION + " cell for this platform; qualified: " +
            ", ".join(qualified_cells()) + "; not qualified: " + "; ".join(blocked))

def hub_build(cells):
    """Returns the hub BUILD file routing every Qt target to its platform cell."""
    lines = []
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

def closure_libraries():
    """Returns the runtime closure sonames a Qt library links against."""
    names = list(ICU_LIBRARIES)
    for package in RUNTIME_PACKAGES:
        for library in package.libraries:
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

def _library(name, spec):
    return [
        "cc_library(",
        '    name = "%s",' % name,
        '    srcs = ["lib/%s"],' % spec.library,
        "    hdrs = glob([%s])," % ", ".join(['"include/%s/**"' % headers for headers in spec.headers]),
        "    includes = [%s]," % ", ".join(['"include"'] + ['"include/%s"' % headers for headers in spec.headers]),
        "    deps = [%s]," % ", ".join(['"%s"' % dep for dep in spec.deps]),
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

def cell_build(modules, rules_cc, unsupported):
    """Returns the BUILD file of one acquired cell for the enabled modules."""
    lines = [
        'load("%s", "cc_binary", "cc_library")' % rules_cc,
        'load("%s", "qt_unsupported")' % unsupported,
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        "filegroup(",
        '    name = "runtime",',
        '    srcs = glob(["lib/*.so*"]),',
        ")",
        "",
        "filegroup(",
        '    name = "licenses",',
        '    srcs = glob(["licenses/**"]),',
        ")",
        "",
        "filegroup(",
        '    name = "sbom",',
        '    srcs = glob(["sbom/*.spdx.json"]),',
        ")",
        "",
        "cc_library(",
        '    name = "closure",',
        "    srcs = [",
    ]
    for name in closure_libraries():
        if not name.startswith("libstdc++"):
            lines.append('        "lib/%s",' % name)
    lines.extend([
        "    ],",
        '    data = [":runtime"],',
        '    linkopts = ["-Wl,--allow-shlib-undefined"],',
        '    visibility = ["//visibility:private"],',
        ")",
        "",
    ])
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
        if name in enabled:
            lines.extend(_library(name, _LIBRARIES[name]))
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
            '    srcs = glob(["plugins/qmllint/*.so"]),',
            ")",
            "",
        ])
    for name in sorted(TOOLS.keys()):
        if name in enabled:
            lines.extend(_tool(name))
    for name in sorted(UNSUPPORTED_TARGETS.keys()):
        lines.extend(_unsupported(name, UNSUPPORTED_TARGETS[name]))
    return "\n".join(lines)

def _extract(ctx, archive, output, links = False):
    command = [str(ctx.path(_EXTRACT + "/7zip/" + EXTRACTOR.binary)), "x", "-y", "-bd", "-o" + output]
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

def _acquire_runtime(ctx):
    roots = {}
    for package in RUNTIME_PACKAGES:
        root = _EXTRACT + "/rpm/" + package.name
        rpm = root + "/" + package.file
        url = runtime_url_for(package, ctx.attr.runtime_mirror)
        ctx.download(url = url, output = rpm, sha256 = package.sha256, canonical_id = canonical_id_for(url))
        _extract(ctx, rpm, root)
        _extract(ctx, root + "/" + package.file.removesuffix(".rpm") + ".cpio", root + "/root", links = True)
        roots[package.name] = root + "/root"
        for library in package.libraries:
            _move_real(ctx, root + "/root/" + library, "lib/" + library.split("/")[-1])
    for package in RUNTIME_PACKAGES:
        if package.license_from == package.name:
            ctx.rename(roots[package.name] + "/" + package.license_dir, "licenses/" + package.name)
    for package in RUNTIME_PACKAGES:
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
    if ctx.os.name != "linux" or ctx.os.arch not in ["amd64", "x86_64"]:
        fail("qt_sdk: the pinned extractor runs on linux x86_64 hosts only, not " + ctx.os.name + " " + ctx.os.arch)
    entry = CELLS[ctx.attr.cell]
    ctx.download_and_extract(
        url = EXTRACTOR.url,
        output = _EXTRACT + "/7zip",
        sha256 = EXTRACTOR.sha256,
        canonical_id = canonical_id_for(EXTRACTOR.url),
    )
    for key in ["icu"] + ctx.attr.modules:
        url = url_for(ctx.attr.cell, key, ctx.attr.mirror)
        archive = _EXTRACT + "/" + entry.archives[key].archive
        ctx.download(url = url, output = archive, sha256 = entry.archives[key].sha256, canonical_id = canonical_id_for(url))
        _extract(ctx, archive, _EXTRACT + "/icu" if key == "icu" else ".", links = True)
    for library in ICU_LIBRARIES:
        _move_real(ctx, _EXTRACT + "/icu/" + library, "lib/" + library)
    _acquire_runtime(ctx)
    for key in sorted(LICENSE_TEXTS.keys()):
        ctx.download(url = LICENSE_TEXTS[key].url, output = "licenses/" + key, sha256 = LICENSE_TEXTS[key].sha256)
    _check_module(ctx, "Core")
    for module in ctx.attr.modules:
        if not ctx.path("sbom/" + module + "-" + QT_VERSION + ".spdx.json").exists:
            fail("qt_sdk: the pinned archives carry no sbom/" + module + "-" + QT_VERSION + ".spdx.json")
    if "qtdeclarative" in ctx.attr.modules:
        _check_module(ctx, "Qml")
    ctx.delete(_EXTRACT)
    ctx.symlink(ctx.attr._launcher, "launcher/launcher.c")
    for name in sorted(TOOLS.keys()):
        ctx.file("launcher/" + name + ".c", _LAUNCH % (ctx.name, TOOLS[name].path))
    ctx.file("BUILD.bazel", cell_build(
        ctx.attr.modules,
        str(Label("@rules_cc//cc:defs.bzl")),
        str(Label("//qt/sdk:unsupported.bzl")),
    ))

def _qt_hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build(json.decode(ctx.attr.cells)))

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
                "runtime_mirror": attr.string(default = "", doc = "Https mirror of the runtime closure packages."),
                "version": attr.string(default = QT_VERSION, doc = "Qt version; only the pinned one is accepted."),
            },
        ),
    },
)
