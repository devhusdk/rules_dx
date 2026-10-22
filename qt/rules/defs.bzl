"""Qt C++ and QML rules over rules_cc and the pinned Qt code generators."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(
    "//libs/starlark:wrapper.bzl",
    "dx_forwarded_runtime_providers",
    "dx_lcov_merger_attr",
    "dx_quality_sources",
    "dx_symlink_default_info",
    "dx_test_forward_kwargs",
    "dx_test_upstream_kwargs",
)
load("//quality:pipeline.bzl", "upstream_skip_tags")
load("//quality:sources.bzl", "QualitySourcesInfo")

QtInfo = provider(
    doc = "Qt metadata one Qt target exports to its dependents.",
    fields = {
        "metatypes": "depset[File]: collected moc JSON of the target and its deps.",
    },
)

QT_REPOSITORY = "@dx_qt"

QT_SOURCE_SPECS = [
    ("c", ["c", "h"]),
    ("cpp", ["cc", "cpp", "cxx", "hh", "hpp", "hxx"]),
    ("qml", ["qml"]),
]

_CC_SRCS = [".c", ".cc", ".cpp", ".cxx"]

_CC_HDRS = [".h", ".hh", ".hpp", ".hxx"]

_QML_SRCS = [".qml", ".js", ".mjs"]

_GENERATED = "_qt"

_TOOL_ENV = {"LC_ALL": "C.UTF-8"}

def qt_identifier(label):
    """Returns the C identifier rcc and qmltyperegistrar use for one label."""
    text = label.repo_name + "_" + label.package + "_" + label.name
    out = ""
    for char in text.elems():
        out += char if char.isalnum() else "_"
    return "qt_" + out.strip("_")

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

def qt_qmldir(uri, typeinfo, types):
    """Returns the qmldir text for one module and its (name, version, path) types."""
    lines = ["module " + uri]
    if typeinfo != "":
        lines.append("typeinfo " + typeinfo)
    lines.append("prefer :/qt/qml/" + qt_module_path(uri) + "/")
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

def _moc(ctx, include_args, headers):
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
        args.add_all(include_args)
        args.add(header)
        ctx.actions.run(
            executable = ctx.executable.moc,
            arguments = [args],
            inputs = depset([header], transitive = headers),
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
    for form in ctx.files.forms:
        relative = _relative(ctx, form, "form")
        out = ctx.actions.declare_file("ui_" + relative.rsplit(".", 1)[0] + ".h")
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
    ctx.actions.write(qmldir, qt_qmldir(uri, typeinfo, types))
    module_files.append(qmldir)
    entries.append(("qmldir", qmldir))
    files = [f for _, f in entries]
    sources.append(_rcc(ctx, "qml", "/qt/qml/" + qt_module_path(uri), entries, files))
    return sources, module_files

def _qt_generate_impl(ctx):
    include_args, headers = _include_args(ctx.attr.deps)
    sources, metatypes = _moc(ctx, include_args, headers)
    dep_metatypes = depset(transitive = [dep[QtInfo].metatypes for dep in ctx.attr.deps if QtInfo in dep])
    collected = _collect(ctx, metatypes) if len(metatypes) > 0 else None
    public = _uic(ctx)
    module_files = []
    if ctx.attr.uri != "":
        generated, module_files = _qml_module(ctx, collected, dep_metatypes)
        sources.extend(generated)
    if len(ctx.files.resources) > 0:
        prefix = ctx.attr.resource_prefix.rstrip("/") or "/"
        entries = [(_relative(ctx, f, "resource"), f) for f in ctx.files.resources]
        sources.append(_rcc(ctx, "resources", prefix, entries, ctx.files.resources))
    return [
        DefaultInfo(files = depset(module_files)),
        OutputGroupInfo(
            qt_hdrs = depset(public),
            qt_srcs = depset(sources),
        ),
        QtInfo(metatypes = depset([collected] if collected != None else [], transitive = [dep_metatypes])),
    ]

_qt_generate = rule(
    implementation = _qt_generate_impl,
    doc = "Runs moc, uic, rcc and qmltyperegistrar for one Qt target.",
    attrs = {
        "base": attr.string(mandatory = True, doc = "Name of the public target."),
        "deps": attr.label_list(doc = "C++ and Qt dependencies whose include context moc reads."),
        "forms": attr.label_list(allow_files = [".ui"], doc = "Qt Designer forms for uic."),
        "metatypes": attr.label_list(allow_files = [".json"], doc = "Qt module metatypes for qmltyperegistrar."),
        "moc": attr.label(cfg = "exec", executable = True, doc = "moc executable."),
        "moc_hdrs": attr.label_list(allow_files = _CC_HDRS, doc = "Headers moc reads."),
        "qml_srcs": attr.label_list(allow_files = _QML_SRCS, doc = "QML and JavaScript module sources."),
        "qmltyperegistrar": attr.label(cfg = "exec", executable = True, doc = "qmltyperegistrar executable."),
        "rcc": attr.label(cfg = "exec", executable = True, doc = "rcc executable."),
        "resource_prefix": attr.string(default = "/", doc = "Resource path prefix."),
        "resources": attr.label_list(allow_files = True, doc = "Files compiled into Qt resources."),
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

def _run_environment(ctx):
    environment = {}
    inherited = []
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
    out = [
        dx_symlink_default_info(ctx, "qt_*", ctx.runfiles(files = ctx.files.qml_imports)),
        _run_environment(ctx),
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
    "qml_imports": attr.label(allow_files = True, doc = "Qt QML import tree put on QML_IMPORT_PATH."),
    "upstream": attr.label(mandatory = True, providers = [CcInfo], doc = "Private rules_cc executable."),
})

_qt_binary_forward = rule(
    implementation = _qt_executable_forward_impl,
    doc = "Forwards a Qt executable and owns only its hand-written sources.",
    executable = True,
    provides = [DefaultInfo, QualitySourcesInfo, RunEnvironmentInfo],
    attrs = _EXECUTABLE_ATTRS,
)

_qt_forward_test = rule(
    implementation = _qt_executable_forward_impl,
    doc = "Forwards a Qt test and owns only its hand-written sources.",
    test = True,
    provides = [DefaultInfo, QualitySourcesInfo, RunEnvironmentInfo],
    attrs = dict(_EXECUTABLE_ATTRS, **dx_lcov_merger_attr()),
)

def _upstream_tags(tags):
    if tags == None:
        return []
    return upstream_skip_tags(tags) + (["manual"] if "manual" in tags else [])

def _include_copts():
    repository = native.repository_name().lstrip("@")
    package = native.package_name()
    parts = (["external", repository] if repository != "" else []) + ([package] if package != "" else [])
    directory = "/".join(parts) if len(parts) > 0 else "."
    return ["-fPIC", "-I" + directory, "-I$(BINDIR)/" + directory]

def _generate(name, qt, deps, moc_hdrs, forms, resources, resource_prefix, qml_srcs, uri, version, tags, testonly, target_compatible_with):
    uses_qml = uri != None
    _qt_generate(
        name = name + "_qt",
        base = name,
        deps = deps,
        forms = forms,
        metatypes = [qt + "//:metatypes"] if uses_qml else [],
        moc = qt + "//:moc",
        moc_hdrs = moc_hdrs,
        qml_srcs = qml_srcs,
        qmltyperegistrar = qt + "//:qmltyperegistrar" if uses_qml else None,
        rcc = qt + "//:rcc",
        resource_prefix = resource_prefix,
        resources = resources,
        uic = qt + "//:uic",
        uri = uri if uses_qml else "",
        version = version,
        tags = (tags or []) + ["manual"],
        testonly = testonly,
        target_compatible_with = target_compatible_with,
        visibility = ["//visibility:private"],
    )
    native.filegroup(
        name = name + "_qt_srcs",
        srcs = [":" + name + "_qt"],
        output_group = "qt_srcs",
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )
    native.filegroup(
        name = name + "_qt_hdrs",
        srcs = [":" + name + "_qt"],
        output_group = "qt_hdrs",
        tags = ["manual"],
        testonly = testonly,
        visibility = ["//visibility:private"],
    )

def _qt_deps(qt, deps, modules, forms):
    out = [qt + "//:" + module for module in modules]
    if forms:
        out.append(qt + "//:widgets")
    return deps + out

def _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, deps, modules, qt, qml_srcs, uri, version, copts, visibility, kwargs):
    effective_deps = _qt_deps(qt, deps, modules, forms)
    tags = kwargs.pop("tags", None)
    testonly = kwargs.get("testonly", None)
    compatible = kwargs.get("target_compatible_with", None)
    _generate(name, qt, effective_deps, moc_hdrs, forms, resources, resource_prefix, qml_srcs, uri, version, tags, testonly, compatible)
    _cc_library(
        name = name + "_upstream",
        srcs = srcs + [":" + name + "_qt_srcs"],
        hdrs = hdrs + moc_hdrs + [":" + name + "_qt_hdrs"],
        copts = copts + _include_copts(),
        deps = effective_deps,
        alwayslink = True,
        tags = _upstream_tags(tags),
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
        srcs = srcs,
        hdrs = hdrs,
        moc_hdrs = moc_hdrs,
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
        deps = [],
        modules = ["core"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a C++ library whose moc, uic and rcc outputs are generated from declared inputs."""
    _library(name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, deps, modules, qt, [], None, "1.0", copts, visibility, kwargs)

def qt_qml_module(
        name,
        uri,
        version = "1.0",
        qml_srcs = [],
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        resources = [],
        resource_prefix = "/",
        deps = [],
        modules = ["qml"],
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QML module with its qmldir, qmltypes, resources and C++ types."""
    _library(name, srcs, hdrs, moc_hdrs, [], resources, resource_prefix, deps, modules, qt, qml_srcs, uri, version, copts, visibility, kwargs)

def _executable(rule_kind, name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, deps, modules, qt, qml_imports, copts, visibility, kwargs):
    effective_deps = _qt_deps(qt, deps, modules, forms)
    test = rule_kind == "test"
    upstream_kwargs = dx_test_upstream_kwargs(kwargs) if test else dict(kwargs)
    tags = upstream_kwargs.pop("tags", None)
    upstream_kwargs.pop("visibility", None)
    _generate(name, qt, effective_deps, moc_hdrs, forms, resources, resource_prefix, [], None, "1.0", tags, True if test else kwargs.get("testonly", None), kwargs.get("target_compatible_with", None))
    upstream_kwargs.setdefault("linkstatic", False)
    upstream = _cc_test if test else _cc_binary
    upstream(
        name = name + "_upstream",
        srcs = srcs + hdrs + moc_hdrs + [":" + name + "_qt_srcs", ":" + name + "_qt_hdrs"],
        copts = copts + _include_copts(),
        deps = effective_deps,
        tags = tags if test else _upstream_tags(tags),
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward = dx_test_forward_kwargs(kwargs) if test else {}
    if not test:
        for key in ["aspect_hints", "tags", "testonly", "target_compatible_with"]:
            if kwargs.get(key, None) != None:
                forward[key] = kwargs[key]
    forward_rule = _qt_forward_test if test else _qt_binary_forward
    forward_rule(
        name = name,
        srcs = srcs,
        hdrs = hdrs,
        moc_hdrs = moc_hdrs,
        deps = deps,
        qml_imports = qt + "//:qml_imports" if (qml_imports if qml_imports != None else "qml" in modules) else None,
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
        deps = [],
        modules = ["core"],
        qt = QT_REPOSITORY,
        qml_imports = None,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a Qt executable over cc_binary."""
    _executable("binary", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, deps, modules, qt, qml_imports, copts, visibility, kwargs)

def qt_cc_test(
        name,
        srcs = [],
        hdrs = [],
        moc_hdrs = [],
        forms = [],
        resources = [],
        resource_prefix = "/",
        deps = [],
        modules = ["core", "test"],
        qt = QT_REPOSITORY,
        qml_imports = None,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a QtTest executable over cc_test."""
    _executable("test", name, srcs, hdrs, moc_hdrs, forms, resources, resource_prefix, deps, modules, qt, qml_imports, copts, visibility, kwargs)
