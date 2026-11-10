"""The rule that runs moc, uic, rcc, lrelease and qmltyperegistrar for one Qt target."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(":constants.bzl", "QT_CC_HDRS", "QT_GENERATED", "QT_QML_SRCS", "QT_TOOL_ENV")
load(":providers.bzl", "QtInfo")
load(":text.bzl", "qt_identifier", "qt_module_path", "qt_package_path", "qt_qmldir", "qt_qrc", "qt_relative", "qt_stem", "qt_version_parts")
load(":validate.bzl", "qt_uri_error")

def _base(ctx):
    return ctx.attr.base

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
        relative = qt_relative(ctx, header, "moc header")
        out = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/moc_" + relative.replace("/", "_").rsplit(".", 1)[0] + ".cpp")
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
            env = QT_TOOL_ENV,
            mnemonic = "QtMoc",
            progress_message = "Running moc on %{input}",
        )
        sources.append(out)
        metatypes.append(json)
    return sources, metatypes

def _collect(ctx, metatypes):
    out = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/" + _base(ctx) + "_metatypes.json")
    args = ctx.actions.args()
    args.add("--collect-json")
    args.add("-o", out)
    args.add_all(metatypes)
    ctx.actions.run(
        executable = ctx.executable.moc,
        arguments = [args],
        inputs = metatypes,
        outputs = [out],
        env = QT_TOOL_ENV,
        mnemonic = "QtMocCollect",
        progress_message = "Collecting moc metatypes for %{label}",
    )
    return out

def _uic(ctx):
    headers = []
    names = {}
    for form in ctx.files.forms:
        qt_relative(ctx, form, "form")
        if qt_stem(form) in names:
            fail(str(ctx.label) + ": forms '" + names[qt_stem(form)] + "' and '" + str(form.owner) + "' both produce ui_" + qt_stem(form) + ".h")
        names[qt_stem(form)] = str(form.owner)
        out = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/ui_" + qt_stem(form) + ".h")
        args = ctx.actions.args()
        args.add("-o", out)
        args.add(form)
        ctx.actions.run(
            executable = ctx.executable.uic,
            arguments = [args],
            inputs = [form],
            outputs = [out],
            env = QT_TOOL_ENV,
            mnemonic = "QtUic",
            progress_message = "Running uic on %{input}",
        )
        headers.append(out)
    return headers

def _rcc(ctx, kind, prefix, entries, files):
    name = qt_identifier(ctx.label.same_package_label(_base(ctx))) + "_" + kind
    stem = _base(ctx) + "_" + kind
    qrc = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/" + stem + ".qrc")
    out = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/qrc_" + stem + ".cpp")
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
        env = QT_TOOL_ENV,
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
        registrations = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/" + _base(ctx) + "_qmltyperegistrations.cpp")
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
            env = QT_TOOL_ENV,
            mnemonic = "QtQmlTypeRegistrar",
            progress_message = "Registering QML types of %{label}",
        )
        sources.append(registrations)
        module_files.append(qmltypes)
        entries.append((typeinfo, qmltypes))
    types = []
    for src in ctx.files.qml_srcs:
        relative = qt_relative(ctx, src, "QML source")
        link = ctx.actions.declare_file(root + "/" + relative)
        ctx.actions.symlink(output = link, target_file = src)
        module_files.append(link)
        entries.append((relative, src))
        stem = qt_stem(src)
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
        qt_relative(ctx, ts, "translation")
        if qt_stem(ts) in names:
            fail(str(ctx.label) + ": translations '" + names[qt_stem(ts)] + "' and '" + str(ts.owner) + "' both produce " + qt_stem(ts) + ".qm")
        names[qt_stem(ts)] = str(ts.owner)
        out = ctx.actions.declare_file(QT_GENERATED + "/" + _base(ctx) + "/i18n/" + qt_stem(ts) + ".qm")
        args = ctx.actions.args()
        args.add("-silent")
        args.add(ts)
        args.add("-qm", out)
        ctx.actions.run(
            executable = ctx.executable.lrelease,
            arguments = [args],
            inputs = [ts],
            outputs = [out],
            env = QT_TOOL_ENV,
            mnemonic = "QtLrelease",
            progress_message = "Running lrelease on %{input}",
        )
        entries.append((out.basename, out))
    return entries

def _translation_owners(ctx):
    own = [qt_stem(ts) + "=" + str(ctx.label.same_package_label(ctx.attr.base)) for ts in ctx.files.translations]
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
        entries = [(qt_relative(ctx, f, "resource"), f) for f in ctx.files.resources]
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

qt_generate = rule(
    implementation = _qt_generate_impl,
    doc = "Runs moc, uic, rcc, lrelease and qmltyperegistrar for one Qt target.",
    attrs = {
        "base": attr.string(mandatory = True, doc = "Name of the public target."),
        "deps": attr.label_list(doc = "C++ and Qt dependencies whose include context moc reads."),
        "copts": attr.string_list(doc = "Compiler options whose defines and include paths moc reads."),
        "defines": attr.string_list(doc = "Preprocessor defines moc reads."),
        "forms": attr.label_list(allow_files = [".ui"], doc = "Qt Designer forms for uic."),
        "hdrs": attr.label_list(allow_files = QT_CC_HDRS, doc = "Public headers moc may include."),
        "lrelease": attr.label(cfg = "exec", executable = True, doc = "lrelease executable."),
        "metatypes": attr.label_list(allow_files = [".json"], doc = "Qt module metatypes for qmltyperegistrar."),
        "moc": attr.label(cfg = "exec", executable = True, doc = "moc executable."),
        "moc_hdrs": attr.label_list(allow_files = QT_CC_HDRS, doc = "Headers moc reads."),
        "qml_depends": attr.string_list(doc = "QML modules the qmldir depends on."),
        "qml_srcs": attr.label_list(allow_files = QT_QML_SRCS, doc = "QML and JavaScript module sources."),
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
