"""Dependency tables one Qt cell publishes for qt_bundle."""

QtNeedsInfo = provider(
    doc = "What each shared object of one Qt cell needs at run time.",
    fields = {
        "library": "dict[str, list[str]]: library name to the library names it needs.",
        "plugin": "dict[str, list[str]]: QML plugin path to the library names it needs.",
        "runtime_plugin": "dict[str, list[str]]: Qt runtime plugin path to the library names it needs.",
    },
)

def _qt_needs_impl(ctx):
    library = dict(ctx.attr.library)
    plugin = dict(ctx.attr.plugin)
    runtime_plugin = dict(ctx.attr.runtime_plugin)
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent({
        "library": library,
        "plugin": plugin,
        "runtime_plugin": runtime_plugin,
    }) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        QtNeedsInfo(library = library, plugin = plugin, runtime_plugin = runtime_plugin),
    ]

qt_needs = rule(
    implementation = _qt_needs_impl,
    doc = "Publishes the dependency tables of one Qt cell and writes them as JSON.",
    attrs = {
        "library": attr.string_list_dict(doc = "Library name to the library names it needs."),
        "plugin": attr.string_list_dict(doc = "QML plugin path to the library names it needs."),
        "runtime_plugin": attr.string_list_dict(doc = "Qt runtime plugin path to the library names it needs."),
    },
)
