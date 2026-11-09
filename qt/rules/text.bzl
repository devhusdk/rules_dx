"""Pure text builders for QML modules, qrc files and package paths."""

_PUNCTUATION = " !\"#$%&'()*+,.:;<=>?@[\\]^`{|}~"

def qt_identifier(label):
    """Returns the C identifier rcc and qmltyperegistrar use for one label."""
    out = ""
    for index, part in enumerate([label.repo_name, label.package, label.name]):
        out += "_p" if index > 0 else ""
        for char in part.elems():
            if char.isalnum():
                out += char
            elif char == "_":
                out += "_u"
            elif char == "/":
                out += "_s"
            elif char == "-":
                out += "_d"
            else:
                index = _PUNCTUATION.find(char)
                if index < 0:
                    fail("qt_identifier: label '" + str(label) + "' holds unsupported character '" + char + "'")
                out += "_x%d_" % index
    return "qt" + out

def qt_module_path(uri):
    """Returns the directory a QML module URI lives in below an import root."""
    return "/".join(uri.split("."))

def qt_version_parts(version):
    """Returns the major and minor version of one QML module version string."""
    parts = version.split(".")
    if len(parts) != 2 or not parts[0].isdigit() or not parts[1].isdigit():
        fail("qt_qml_module: version '" + version + "' must be MAJOR.MINOR")
    return (parts[0], parts[1])

def qt_qmldir(uri, typeinfo, types, depends = []):
    """Returns the qmldir text for one module, its (name, version, path) types and dependencies."""
    lines = ["module " + uri]
    if typeinfo != "":
        lines.append("typeinfo " + typeinfo)
    lines.append("prefer :/qt/qml/" + qt_module_path(uri) + "/")
    for dependency in depends:
        lines.append("depends " + dependency)
    for name, version, path in types:
        lines.append(name + " " + version + " " + path)
    return "\n".join(lines) + "\n"

def _xml(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

def qt_qrc(prefix, entries):
    """Returns the .qrc text mapping (alias, path) entries under one prefix."""
    lines = ["<RCC>", "  <qresource prefix=\"" + _xml(prefix) + "\">"]
    for alias, path in entries:
        lines.append("    <file alias=\"" + _xml(alias) + "\">" + _xml(path) + "</file>")
    lines.extend(["  </qresource>", "</RCC>"])
    return "\n".join(lines) + "\n"

def qt_package_path(short_path, package):
    """Returns a short path relative to its package, or None outside it."""
    path = short_path
    if path.startswith("../"):
        path = path[3:].partition("/")[2]
    if package == "":
        return path
    if not path.startswith(package + "/"):
        return None
    return path[len(package) + 1:]

def qt_framework_path(short_path):
    """Returns the path of one Qt framework binary below its framework directory, or None."""
    parts = short_path.split("/")
    for index in range(len(parts) - 3):
        if parts[index].endswith(".framework") and "/".join(parts[index + 1:index + 3]) == "Versions/A":
            return "/".join(parts[index:])
    return None

def qt_stem(f):
    """Returns the basename of one file without its extension."""
    return f.basename[:-(len(f.extension) + 1)] if f.extension else f.basename

def qt_relative(ctx, f, what):
    """Returns the package-relative path of one file, failing for files of another package."""
    path = qt_package_path(f.short_path, ctx.label.package)
    if path == None or f.owner.repo_name != ctx.label.repo_name or f.owner.package != ctx.label.package:
        fail(str(ctx.label) + ": " + what + " '" + str(f.owner) + "' must be in package " + str(ctx.label.package) + "; give it its own target")
    return path
