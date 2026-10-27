"""Writes the JSON spec of one pinned Qt cell."""

load(":pins.bzl", "CELLS")
load(":repos.bzl", "cell_spec", "qualified_cells")

def _qt_cell_json_impl(ctx):
    if ctx.attr.cell not in qualified_cells():
        fail("qt_cell_json: '" + ctx.attr.cell + "' is not one of " + ", ".join(qualified_cells()))
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent(cell_spec(CELLS[ctx.attr.cell])) + "\n")
    return [DefaultInfo(files = depset([out]))]

qt_cell_json = rule(
    implementation = _qt_cell_json_impl,
    doc = "Writes the JSON spec of one pinned Qt cell for qt_sdk.cell.",
    attrs = {
        "cell": attr.string(doc = "Pinned cell to export.", mandatory = True),
    },
)
