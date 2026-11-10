"""Writes the JSON spec of one pinned Qt cell."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")
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

def _qt_cell_data_update_impl(ctx):
    dest = ctx.label.package + "/" + ctx.attr.directory + "/"
    lines = [
        "copy=" + rlocation_path(ctx, generated) + "\t" + dest + generated.basename.removesuffix("_cell.json") + ".json"
        for generated in ctx.files.generated
    ]
    launcher, _, runfiles = write_native_launcher(ctx, "\n".join(lines) + "\n")
    return [DefaultInfo(executable = launcher, runfiles = runfiles.merge(ctx.runfiles(files = ctx.files.generated)))]

qt_cell_data_update = rule(
    implementation = _qt_cell_data_update_impl,
    doc = "Copies the generated cell JSON files into the checked-in data directory.",
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "directory": attr.string(doc = "Package-relative directory that holds the checked-in JSON.", mandatory = True),
        "generated": attr.label_list(doc = "Generated cell JSON files.", allow_files = [".json"], mandatory = True),
    },
    executable = True,
)

def qt_cell_data(name = "cell_data"):
    """Declares the generated cell JSON, its update run target and one diff test per cell."""
    generated = []
    for cell in qualified_cells():
        qt_cell_json(name = cell + "_cell", cell = cell)
        generated.append(":" + cell + "_cell")
        diff_test(
            name = cell + "_cell_diff_test",
            size = "small",
            file1 = "data/" + cell + ".json",
            file2 = ":" + cell + "_cell",
        )
    qt_cell_data_update(name = name + "_update", directory = "data", generated = generated)
