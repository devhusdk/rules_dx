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
    "dx_test_forward_kwargs",
    "dx_test_upstream_kwargs",
    "dx_upstream_tags",
)
load("//qt/sdk:needs.bzl", "QtNeedsInfo")
load("//qt/sdk:pins.bzl", "CELLS", "RUNTIME_PLUGINS")
load("//qt/sdk:repos.bzl", "qualified_cells")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo")

QtInfo = provider(
    doc = "Qt metadata one Qt target exports to its dependents.",
    fields = {
        "metatypes": "depset[File]: collected moc JSON of the target and its deps.",
        "translations": "depset[str]: 'stem=label' of every translation the target and its deps embed.",
    },
)

QT_REPOSITORY = "@dx_qt"

QT_BUNDLE_RUNPATH = "-Wl,--disable-new-dtags,-rpath,$$ORIGIN/resources/lib"

QT_SOURCE_SPECS = [
    ("c", ["c", "h"]),
    ("cpp", ["cc", "cpp", "cxx", "hh", "hpp", "hxx"]),
    ("qml", ["qml"]),
]

_CC_SRCS = [".c", ".cc", ".cpp", ".cxx"]

_CC_HDRS = [".h", ".hh", ".hpp", ".hxx"]

_QML_SRCS = [".qml", ".js", ".mjs"]

_GENERATED = "_qt"

_QML_MODULES = ["qml", "quick", "quick_test"]

_TOOL_ENV = {"LC_ALL": "C.UTF-8"}

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

def qt_uri_error(uri):
    """Returns why a QML module URI is malformed, or an empty string."""
    if uri == "":
        return "uri must not be empty"
    for segment in uri.split("."):
        if segment == "" or not (segment[0].isalpha() or segment[0] == "_"):
            return "uri '" + uri + "' segment '" + segment + "' must start with a letter"
        for char in segment.elems():
            if not (char.isalnum() or char == "_"):
                return "uri '" + uri + "' segment '" + segment + "' holds '" + char + "'"
    return ""

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

def _base(ctx):
    return ctx.attr.base

def _stem(f):
    return f.basename[:-(len(f.extension) + 1)] if f.extension else f.basename

def _relative(ctx, f, what):
    path = qt_package_path(f.short_path, ctx.label.package)
    if path == None or f.owner.repo_name != ctx.label.repo_name or f.owner.package != ctx.label.package:
        fail(str(ctx.label) + ": " + what + " '" + str(f.owner) + "' must be in package " + str(ctx.label.package) + "; give it its own target")
    return path

def _from_execroot(directory, path):
    return "../" * len(directory.split("/")) + path

def _include_args(deps):
    args = []
    headers = []
    for dep in deps:
        if CcInfo not in dep:
            continue
        context = dep[CcInfo].compilation_context
        headers.append(context.headers)
        for directory in context.includes.to_list() + context.system_includes.to_list() + context.quote_includes.to_list():
            args.append("-I" + directory)
        for define in context.defines.to_list():
            args.append("-D" + define)
    return args, headers

def _flag_args(ctx):
    args = []
    for flag in ctx.attr.copts:
        expanded = ctx.expand_make_variables("copts", flag, {})
        if expanded.startswith(("-D", "-U", "-I")):
            args.append(expanded)
    for define in ctx.attr.defines:
        args.append("-D" + define)
    return args

def _moc(ctx, include_args, headers, generated):
    sources = []
    metatypes = []
    for header in ctx.files.moc_hdrs:
        relative = _relative(ctx, header, "moc header")
        out = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/moc_" + relative.replace("/", "_").rsplit(".", 1)[0] + ".cpp")
        json = ctx.actions.declare_file(out.basename + ".json", sibling = out)
        include = header.short_path
        if include.startswith("../"):
            include = include[3:].partition("/")[2]
        args = ctx.actions.args()
        args.add("--output-json")
        args.add("-o", out)
        args.add("-f" + include)
        args.add("-I.")
        args.add("-I" + ctx.bin_dir.path)
        args.add_all(include_args)
        args.add_all(_flag_args(ctx))
        args.add(header)
        ctx.actions.run(
            executable = ctx.executable.moc,
            arguments = [args],
            inputs = depset(ctx.files.moc_hdrs + ctx.files.hdrs + generated, transitive = headers),
            outputs = [out, json],
            env = _TOOL_ENV,
            mnemonic = "QtMoc",
            progress_message = "Running moc on %{input}",
        )
        sources.append(out)
        metatypes.append(json)
    return sources, metatypes

def _collect(ctx, metatypes):
    out = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/" + _base(ctx) + "_metatypes.json")
    args = ctx.actions.args()
    args.add("--collect-json")
    args.add("-o", out)
    args.add_all(metatypes)
    ctx.actions.run(
        executable = ctx.executable.moc,
        arguments = [args],
        inputs = metatypes,
        outputs = [out],
        env = _TOOL_ENV,
        mnemonic = "QtMocCollect",
        progress_message = "Collecting moc metatypes for %{label}",
    )
    return out

def _uic(ctx):
    headers = []
    names = {}
    for form in ctx.files.forms:
        _relative(ctx, form, "form")
        if _stem(form) in names:
            fail(str(ctx.label) + ": forms '" + names[_stem(form)] + "' and '" + str(form.owner) + "' both produce ui_" + _stem(form) + ".h")
        names[_stem(form)] = str(form.owner)
        out = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/ui_" + _stem(form) + ".h")
        args = ctx.actions.args()
        args.add("-o", out)
        args.add(form)
        ctx.actions.run(
            executable = ctx.executable.uic,
            arguments = [args],
            inputs = [form],
            outputs = [out],
            env = _TOOL_ENV,
            mnemonic = "QtUic",
            progress_message = "Running uic on %{input}",
        )
        headers.append(out)
    return headers

def _rcc(ctx, kind, prefix, entries, files):
    name = qt_identifier(ctx.label.same_package_label(_base(ctx))) + "_" + kind
    stem = _base(ctx) + "_" + kind
    qrc = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/" + stem + ".qrc")
    out = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/qrc_" + stem + ".cpp")
    ctx.actions.write(qrc, qt_qrc(prefix, [(alias, _from_execroot(qrc.dirname, f.path)) for alias, f in entries]))
    args = ctx.actions.args()
    args.add("--format-version", "1")
    args.add("--no-zstd")
    args.add("--name", name)
    args.add("-o", out)
    args.add(qrc)
    ctx.actions.run(
        executable = ctx.executable.rcc,
        arguments = [args],
        inputs = [qrc] + files,
        outputs = [out],
        env = _TOOL_ENV,
        mnemonic = "QtRcc",
        progress_message = "Running rcc for %{label}",
    )
    return out

def _qml_module(ctx, collected, dep_metatypes):
    uri = ctx.attr.uri
    error = qt_uri_error(uri)
    if error != "":
        fail(str(ctx.label) + ": " + error)
    for dependency in ctx.attr.qml_depends:
        error = qt_uri_error(dependency)
        if error != "":
            fail(str(ctx.label) + ": qml_depends " + error)
    major, minor = qt_version_parts(ctx.attr.version)
    root = _base(ctx) + "_qml/" + qt_module_path(uri)
    sources = []
    module_files = []
    entries = []
    typeinfo = ""
    if collected != None:
        typeinfo = _base(ctx) + ".qmltypes"
        qmltypes = ctx.actions.declare_file(root + "/" + typeinfo)
        registrations = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/" + _base(ctx) + "_qmltyperegistrations.cpp")
        foreign = ctx.files.metatypes + dep_metatypes.to_list()
        args = ctx.actions.args()
        args.add("--import-name", uri)
        args.add("--major-version", major)
        args.add("--minor-version", minor)
        args.add("--generate-qmltypes", qmltypes)
        args.add_joined("--foreign-types", foreign, join_with = ",")
        args.add("-o", registrations)
        args.add(collected)
        ctx.actions.run(
            executable = ctx.executable.qmltyperegistrar,
            arguments = [args],
            inputs = [collected] + foreign,
            outputs = [qmltypes, registrations],
            env = _TOOL_ENV,
            mnemonic = "QtQmlTypeRegistrar",
            progress_message = "Registering QML types of %{label}",
        )
        sources.append(registrations)
        module_files.append(qmltypes)
        entries.append((typeinfo, qmltypes))
    types = []
    for src in ctx.files.qml_srcs:
        relative = _relative(ctx, src, "QML source")
        link = ctx.actions.declare_file(root + "/" + relative)
        ctx.actions.symlink(output = link, target_file = src)
        module_files.append(link)
        entries.append((relative, src))
        stem = _stem(src)
        if src.extension == "qml" and stem[:1].isupper():
            types.append((stem, ctx.attr.version, relative))
    qmldir = ctx.actions.declare_file(root + "/qmldir")
    ctx.actions.write(qmldir, qt_qmldir(uri, typeinfo, types, ctx.attr.qml_depends))
    module_files.append(qmldir)
    entries.append(("qmldir", qmldir))
    files = [f for _, f in entries]
    sources.append(_rcc(ctx, "qml", "/qt/qml/" + qt_module_path(uri), entries, files))
    return sources, module_files

def _lrelease(ctx):
    entries = []
    names = {}
    for ts in ctx.files.translations:
        _relative(ctx, ts, "translation")
        if _stem(ts) in names:
            fail(str(ctx.label) + ": translations '" + names[_stem(ts)] + "' and '" + str(ts.owner) + "' both produce " + _stem(ts) + ".qm")
        names[_stem(ts)] = str(ts.owner)
        out = ctx.actions.declare_file(_GENERATED + "/" + _base(ctx) + "/i18n/" + _stem(ts) + ".qm")
        args = ctx.actions.args()
        args.add("-silent")
        args.add(ts)
        args.add("-qm", out)
        ctx.actions.run(
            executable = ctx.executable.lrelease,
            arguments = [args],
            inputs = [ts],
            outputs = [out],
            env = _TOOL_ENV,
            mnemonic = "QtLrelease",
            progress_message = "Running lrelease on %{input}",
        )
        entries.append((out.basename, out))
    return entries

def _translation_owners(ctx):
    own = [_stem(ts) + "=" + str(ctx.label.same_package_label(ctx.attr.base)) for ts in ctx.files.translations]
    translations = depset(own, transitive = [dep[QtInfo].translations for dep in ctx.attr.deps if QtInfo in dep])
    owners = {}
    for entry in translations.to_list():
        stem, _, owner = entry.partition("=")
        if stem in owners and owners[stem] != owner:
            fail(str(ctx.label) + ": " + owners[stem] + " and " + owner + " both embed :/i18n/" + stem + ".qm")
        owners[stem] = owner
    return translations

def _qt_generate_impl(ctx):
    include_args, headers = _include_args(ctx.attr.deps)
    translations = _translation_owners(ctx)
    public = _uic(ctx)
    sources, metatypes = _moc(ctx, include_args, headers, public)
    dep_metatypes = depset(transitive = [dep[QtInfo].metatypes for dep in ctx.attr.deps if QtInfo in dep])
    collected = _collect(ctx, metatypes) if len(metatypes) > 0 else None
    module_files = []
    if ctx.attr.uri != "":
        generated, module_files = _qml_module(ctx, collected, dep_metatypes)
        sources.extend(generated)
    if len(ctx.files.resources) > 0:
        prefix = ctx.attr.resource_prefix.rstrip("/") or "/"
        entries = [(_relative(ctx, f, "resource"), f) for f in ctx.files.resources]
        sources.append(_rcc(ctx, "resources", prefix, entries, ctx.files.resources))
    if len(ctx.files.translations) > 0:
        entries = _lrelease(ctx)
        sources.append(_rcc(ctx, "i18n", "/i18n", entries, [f for _, f in entries]))
    return [
        DefaultInfo(files = depset(module_files)),
        OutputGroupInfo(
            qt_hdrs = depset(public),
            qt_srcs = depset(sources),
        ),
        QtInfo(metatypes = depset([collected] if collected != None else [], transitive = [dep_metatypes]), translations = translations),
    ]

_qt_generate = rule(
    implementation = _qt_generate_impl,
    doc = "Runs moc, uic, rcc, lrelease and qmltyperegistrar for one Qt target.",
    attrs = {
        "base": attr.string(mandatory = True, doc = "Name of the public target."),
        "deps": attr.label_list(doc = "C++ and Qt dependencies whose include context moc reads."),
        "copts": attr.string_list(doc = "Compiler options whose defines and include paths moc reads."),
        "defines": attr.string_list(doc = "Preprocessor defines moc reads."),
        "forms": attr.label_list(allow_files = [".ui"], doc = "Qt Designer forms for uic."),
        "hdrs": attr.label_list(allow_files = _CC_HDRS, doc = "Public headers moc may include."),
        "lrelease": attr.label(cfg = "exec", executable = True, doc = "lrelease executable."),
        "metatypes": attr.label_list(allow_files = [".json"], doc = "Qt module metatypes for qmltyperegistrar."),
        "moc": attr.label(cfg = "exec", executable = True, doc = "moc executable."),
        "moc_hdrs": attr.label_list(allow_files = _CC_HDRS, doc = "Headers moc reads."),
        "qml_depends": attr.string_list(doc = "QML modules the qmldir depends on."),
        "qml_srcs": attr.label_list(allow_files = _QML_SRCS, doc = "QML and JavaScript module sources."),
        "qmltyperegistrar": attr.label(cfg = "exec", executable = True, doc = "qmltyperegistrar executable."),
        "rcc": attr.label(cfg = "exec", executable = True, doc = "rcc executable."),
        "resource_prefix": attr.string(default = "/", doc = "Resource path prefix."),
        "resources": attr.label_list(allow_files = True, doc = "Files compiled into Qt resources."),
        "translations": attr.label_list(allow_files = [".ts"], doc = "Qt Linguist sources compiled into resources."),
        "uic": attr.label(cfg = "exec", executable = True, doc = "uic executable."),
        "uri": attr.string(doc = "QML module URI."),
        "version": attr.string(default = "1.0", doc = "QML module version."),
    },
)

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
    "hdrs": attr.label_list(allow_files = _CC_HDRS, doc = "Owned headers."),
    "moc_hdrs": attr.label_list(allow_files = _CC_HDRS, doc = "Owned headers moc reads."),
    "qml_srcs": attr.label_list(allow_files = _QML_SRCS, doc = "Owned QML sources."),
    "srcs": attr.label_list(allow_files = _CC_SRCS + _CC_HDRS, doc = "Owned C++ sources."),
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

def _links_gui(upstream):
    for f in upstream[DefaultInfo].default_runfiles.files.to_list():
        if f.basename == "libQt6Gui.so.6" and "_solib_" in f.short_path:
            return True
        if f.basename == "QtGui" and _FRAMEWORK in f.short_path:
            return True
    return False

def _directory(files, prefix):
    for f in files:
        if f.basename.startswith(prefix):
            return f.short_path.rpartition("/")[0]
    fail("no " + prefix + " in " + str([f.short_path for f in files]))

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
    environment = {}
    if len(ctx.files.plugins) > 0:
        environment["QT_PLUGIN_PATH"] = _plugin_path(ctx.files.plugins)
    inherited = []
    if gui:
        if len([f for f in ctx.files.fonts if f.basename == "fonts.conf"]) > 0:
            environment["FONTCONFIG_PATH"] = _directory(ctx.files.fonts, "fonts.conf")
        environment["QT_QPA_PLATFORM_PLUGIN_PATH"] = _directory(ctx.files.platforms, "libqoffscreen.")
        if ctx.attr._headless:
            environment["QT_QPA_PLATFORM"] = "offscreen"
            environment["QT_QUICK_BACKEND"] = "software"
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
        _run_environment(ctx, gui),
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
    attrs = dict(_EXECUTABLE_ATTRS, _headless = attr.bool(default = False), **dx_forward_data_attr()),
)

_qt_forward_test = rule(
    implementation = _qt_executable_forward_impl,
    doc = "Forwards a Qt test and owns only its hand-written sources.",
    test = True,
    provides = [DefaultInfo, QualitySourcesInfo, RunEnvironmentInfo],
    attrs = dict(_EXECUTABLE_ATTRS, _headless = attr.bool(default = True), **(dx_lcov_merger_attr() | dx_forward_data_attr())),
)

def qt_framework_path(short_path):
    """Returns the path of one Qt framework binary below its framework directory, or None."""
    parts = short_path.split("/")
    for index in range(len(parts) - 3):
        if parts[index].endswith(".framework") and "/".join(parts[index + 1:index + 3]) == "Versions/A":
            return "/".join(parts[index:])
    return None

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
        "@platforms//os:macos": [],
        "//conditions:default": [QT_BUNDLE_RUNPATH],
    })

def _include_copts(name):
    repository = native.repository_name().lstrip("@")
    package = native.package_name()
    parts = (["external", repository] if repository != "" else []) + ([package] if package != "" else [])
    directory = "/".join(parts) if len(parts) > 0 else "."
    return ["-fPIC", "-I" + directory, "-I$(BINDIR)/" + directory, "-I$(BINDIR)/" + directory + "/" + _GENERATED + "/" + name]

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

def _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, qml_srcs, uri, qml_depends, version, copts, visibility, kwargs, owned = True):
    effective_deps = _qt_deps(qt, deps, modules, forms)
    tags = kwargs.pop("tags", None)
    testonly = kwargs.get("testonly", None)
    compatible = kwargs.get("target_compatible_with", None)
    _generate(name, qt, effective_deps, hdrs, moc_hdrs, forms, copts + _include_copts(name), kwargs.get("defines", []) + kwargs.get("local_defines", []), resources, resource_prefix, translations, qml_srcs, uri, qml_depends, version, tags, testonly, compatible)
    _cc_library(
        name = name + "_upstream",
        srcs = srcs + [":" + name + "_qt_srcs"],
        hdrs = hdrs + moc_hdrs + [":" + name + "_qt_hdrs"],
        copts = copts + _include_copts(name),
        deps = effective_deps,
        alwayslink = True,
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
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = [],
        modules = ["core"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a C++ library whose moc, uic and rcc outputs are generated from declared inputs."""
    _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, [], None, [], "1.0", copts, visibility, kwargs)

def qt_qml_module(
        name,
        uri,
        version = "1.0",
        qml_srcs = [],
        qml_depends = [],
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = [],
        modules = ["qml"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QML module with its qmldir, qmltypes, resources and C++ types."""
    _library(name, srcs, hdrs, moc_hdrs, [], resources, resource_prefix, translations, deps, modules, qt, qml_srcs, uri, qml_depends, version, copts, visibility, kwargs)

def _uses_qml(modules):
    return len([module for module in modules if module in _QML_MODULES]) > 0

def _executable(rule_kind, name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, qml_imports, plugins, copts, visibility, kwargs):
    for plugin in plugins:
        if plugin not in RUNTIME_PLUGINS:
            fail("qt_cc_" + rule_kind + ": plugin '" + plugin + "' is not one of " + ", ".join(sorted(RUNTIME_PLUGINS.keys())))
    effective_deps = _qt_deps(qt, deps, modules, forms)
    test = rule_kind == "test"
    upstream_kwargs = dx_test_upstream_kwargs(kwargs) if test else dict(kwargs)
    tags = upstream_kwargs.pop("tags", None)
    upstream_kwargs.pop("visibility", None)
    _generate(name, qt, effective_deps, hdrs, moc_hdrs, forms, copts + _include_copts(name), kwargs.get("defines", []) + kwargs.get("local_defines", []), resources, resource_prefix, translations, [], None, [], "1.0", tags, True if test else kwargs.get("testonly", None), kwargs.get("target_compatible_with", None))
    upstream_kwargs.setdefault("linkstatic", False)
    if not test:
        upstream_kwargs["linkopts"] = upstream_kwargs.get("linkopts", []) + _bundle_runpath()
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
        deps = effective_deps + [":" + name + "_qt_frameworks"],
        tags = upstream_tags(tags),
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
        qml_imports = qt + "//:qml_imports" if (qml_imports if qml_imports != None else _uses_qml(modules)) else None,
        upstream = ":" + name + "_upstream",
        visibility = visibility,
        **forward
    )

def qt_cc_binary(
        name,
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = [],
        modules = ["core"],
        qt = QT_REPOSITORY,
        qml_imports = None,
        plugins = [],
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a Qt executable over cc_binary."""
    _executable("binary", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, qml_imports, plugins, copts, visibility, kwargs)

def qt_cc_test(
        name,
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        translations = [],
        deps = [],
        modules = ["core", "test"],
        qt = QT_REPOSITORY,
        qml_imports = None,
        plugins = [],
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QtTest executable over cc_test."""
    _executable("test", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, translations, deps, modules, qt, qml_imports, plugins, copts, visibility, kwargs)

_BRIDGE_TOOL = Label("//qt/rust:qt_rust_bridge")

_CXX_QT_FILES = Label("//qt/rust:cxx_qt_runtime_files")

def _bridge(ctx, mode, src, outputs, include = None):
    args = ctx.actions.args()
    args.add(mode)
    args.add(src)
    if include != None:
        args.add(include)
    args.add_all(outputs)
    ctx.actions.run(
        executable = ctx.executable.generator,
        arguments = [args],
        inputs = [src],
        outputs = outputs,
        env = _TOOL_ENV,
        mnemonic = "QtRustBridge",
        progress_message = "Generating C++ for %{input}",
    )

def _qt_rust_bridge_impl(ctx):
    headers = []
    sources = []
    qobjects = []
    for src in ctx.files.bridges:
        stem = _relative(ctx, src, "bridge")[:-len(".rs")]
        if "/" in stem:
            fail(str(ctx.label) + ": bridge '" + stem + ".rs' must sit in the package directory, not a subdirectory")
        qobject_header = ctx.actions.declare_file(stem + ".cxxqt.h")
        qobject_source = ctx.actions.declare_file(stem + ".cxxqt.cpp")
        header = ctx.actions.declare_file(stem + ".cxx.h")
        source = ctx.actions.declare_file(stem + ".cxx.cpp")
        include = qt_package_path(src.short_path, "")[:-len(".rs")]
        _bridge(ctx, "qt", src, [qobject_header, qobject_source, header, source], include)
        headers.extend([qobject_header, header])
        sources.extend([qobject_source, source])
        qobjects.append(qobject_header)
    return [
        DefaultInfo(files = depset(headers + sources)),
        OutputGroupInfo(
            qt_rust_hdrs = depset(headers),
            qt_rust_moc_hdrs = depset(qobjects),
            qt_rust_srcs = depset(sources),
        ),
    ]

_qt_rust_bridge = rule(
    implementation = _qt_rust_bridge_impl,
    doc = "Generates the C++ side of CXX-Qt bridges.",
    attrs = {
        "bridges": attr.label_list(allow_files = [".rs"], allow_empty = False, mandatory = True, doc = "Rust files holding one #[cxx_qt::bridge] module each."),
        "generator": attr.label(cfg = "exec", executable = True, default = _BRIDGE_TOOL, doc = "Bridge generator."),
    },
)

def _qt_rust_runtime_files_impl(ctx):
    root = ctx.label.name + "/"
    headers = []
    sources = []
    cxx_h = ctx.actions.declare_file(root + "rust/cxx.h")
    args = ctx.actions.args()
    args.add("header")
    args.add(cxx_h)
    ctx.actions.run(
        executable = ctx.executable.generator,
        arguments = [args],
        outputs = [cxx_h],
        env = _TOOL_ENV,
        mnemonic = "QtRustBridge",
        progress_message = "Writing rust/cxx.h",
    )
    headers.append(cxx_h)
    for f in ctx.files.files:
        if f.extension == "h":
            out = ctx.actions.declare_file(root + "cxx-qt/" + f.basename)
            ctx.actions.symlink(output = out, target_file = f)
            headers.append(out)
        elif f.extension == "cpp":
            out = ctx.actions.declare_file(root + f.basename)
            ctx.actions.symlink(output = out, target_file = f)
            sources.append(out)
        else:
            header = ctx.actions.declare_file(root + _stem(f) + ".cxx.h")
            source = ctx.actions.declare_file(root + _stem(f) + ".cxx.cpp")
            _bridge(ctx, "cxx", f, [header, source])
            headers.append(header)
            sources.append(source)
    return [
        DefaultInfo(files = depset(headers + sources)),
        OutputGroupInfo(
            qt_rust_hdrs = depset(headers),
            qt_rust_srcs = depset(sources),
        ),
    ]

_qt_rust_runtime_files = rule(
    implementation = _qt_rust_runtime_files_impl,
    doc = "Stages the CXX-Qt runtime headers and generates its CXX bridges.",
    attrs = {
        "files": attr.label(allow_files = [".h", ".cpp", ".rs"], default = _CXX_QT_FILES, doc = "CXX-Qt crate headers, C++ sources and bridges."),
        "generator": attr.label(cfg = "exec", executable = True, default = _BRIDGE_TOOL, doc = "Bridge generator."),
    },
)

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
        copts = ["-fPIC"],
        includes = [name + "_files"],
        deps = [core],
        alwayslink = True,
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
    testonly = kwargs.get("testonly", None)
    shared = {}
    for key in ["testonly", "target_compatible_with"]:
        if kwargs.get(key, None) != None:
            shared[key] = kwargs[key]
    _qt_rust_bridge(
        name = name + "_bridge",
        bridges = bridges,
        generator = generator or _BRIDGE_TOOL,
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
    )

def _soname(basename):
    stem, separator, version = basename.rpartition(".so.")
    return separator != "" and stem != "" and version.isdigit()

def qt_consumer_identity(cell):
    """Returns the consumer_archive arch and min_runtime of one qualified Qt cell."""
    runtime = CELLS[cell].runtime
    return struct(arch = runtime.arch, min_runtime = "glibc-" + runtime.glibc)

# Qualified cells with a glibc runtime identity. Non-glibc cells (macos_arm64)
# need a per-OS runtime field first (#3301).
_CONSUMER_CELLS = [
    cell
    for cell in qualified_cells()
    if CELLS[cell].runtime != None and hasattr(CELLS[cell].runtime, "glibc")
]

def _cell_setting(cell):
    return Label("//qt/platforms:cell_" + cell)

# target_compatible_with for targets that build against @dx_qt: every glibc cell.
QT_COMPATIBLE_WITH = select(
    {_cell_setting(cell): [] for cell in _CONSUMER_CELLS} |
    {"//conditions:default": [Label("@platforms//:incompatible")]},
)

# consumer_archive arch of the qualified cell the build targets.
QT_CONSUMER_ARCH = select(
    {_cell_setting(cell): qt_consumer_identity(cell).arch for cell in _CONSUMER_CELLS} |
    {"//conditions:default": ""},
)

# consumer_archive min_runtime of the qualified cell the build targets.
QT_CONSUMER_MIN_RUNTIME = select(
    {_cell_setting(cell): qt_consumer_identity(cell).min_runtime for cell in _CONSUMER_CELLS} |
    {"//conditions:default": ""},
)

def qt_bundle_members(runfiles, licenses, modules, needs):
    """Returns the sorted (member, short path) pairs one bundle stages."""
    roots = [path.rpartition("/")[0] for path in runfiles if path.rpartition("/")[2] == "builtins.qmltypes"]
    if len(roots) > 1:
        fail("qt_bundle: the binary carries " + str(len(roots)) + " QML import trees")
    root = roots[0] + "/" if len(roots) == 1 else None
    if modules == None:
        modules = ["QML", "QtQml"] if root != None else []
    if len(modules) > 0 and root == None:
        fail("qt_bundle: qml_modules " + str(modules) + " need a binary with the QML import tree")
    members = {}
    shipped = {}
    libraries = {}
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
        name = path.rpartition("/")[2]
        if path.rpartition("/")[0].endswith("/fonts") and (name == "fonts.conf" or name.endswith(".ttf")):
            members["fonts/" + name] = path
            continue
        plugin = [key for key in needs.runtime_plugin if path.endswith("/plugins/" + key)]
        if len(plugin) == 1:
            members["plugins/" + plugin[0]] = path
            roots.extend(needs.runtime_plugin[plugin[0]])
            continue
        if _soname(name) or name.endswith(".so"):
            libraries.setdefault(name, path)
            if linked:
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
    modules = ctx.attr.qml_modules if ctx.attr.qml_modules_set else None
    staged = []
    for member, path in qt_bundle_members([f.short_path for f in runfiles], [f.short_path for f in ctx.files.license_files], modules, ctx.attr.needs[QtNeedsInfo]):
        out = ctx.actions.declare_file(ctx.label.name + "/" + member)
        ctx.actions.symlink(output = out, target_file = files[path])
        staged.append(out)
    return [DefaultInfo(files = depset(staged))]

_qt_bundle = rule(
    implementation = _qt_bundle_impl,
    doc = "Stages the linked shared libraries, their dependencies, QML modules, plugins and licenses one Qt binary runs with.",
    attrs = {
        "binary": attr.label(mandatory = True, providers = [RunEnvironmentInfo], doc = "qt_cc_binary to bundle."),
        "display_plugins": attr.label_list(allow_files = True, doc = "Display platform plugin files to ship."),
        "license_files": attr.label_list(allow_files = True, doc = "License texts below a licenses directory."),
        "needs": attr.label(mandatory = True, providers = [QtNeedsInfo], doc = "Dependency tables of the Qt cell the binary runs on."),
        "qml_modules": attr.string_list(doc = "QML import modules to ship."),
        "qml_modules_set": attr.bool(doc = "Whether qml_modules was given."),
        "runtime_plugins": attr.label_list(allow_files = True, doc = "Qt runtime plugin files to ship."),
    },
)

def qt_bundle(name, binary, qml_modules = None, licenses = [], display = [], plugins = [], qt = QT_REPOSITORY, **kwargs):
    """Stages a qt_cc_binary runtime for consumer_archive resources."""
    for platform in display:
        if platform not in ["wayland", "xcb"]:
            fail("qt_bundle: display '" + platform + "' is not one of wayland, xcb")
    for plugin in plugins:
        if plugin not in RUNTIME_PLUGINS:
            fail("qt_bundle: plugin '" + plugin + "' is not one of " + ", ".join(sorted(RUNTIME_PLUGINS.keys())))
    _qt_bundle(
        name = name,
        binary = binary,
        display_plugins = [qt + "//:display_" + platform for platform in display],
        license_files = [qt + "//:licenses"] + licenses,
        needs = qt + "//:needs",
        qml_modules = qml_modules or [],
        qml_modules_set = qml_modules != None,
        runtime_plugins = [qt + "//:plugins_" + plugin for plugin in plugins],
        **kwargs
    )
