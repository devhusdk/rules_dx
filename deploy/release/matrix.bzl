"""Release matrix for the dx standalone archives: one cell per OS and CPU."""

load("//deploy/rules:archive.bzl", "archive_filenames", "consumer_archive")
load(":inventory.bzl", "shipped_inventory")
load(":notice.bzl", "notice_bundle")

RELEASE_MATRIX_SCHEMA = "release-matrix/v1"

def _cell(os, arch, min_runtime):
    """Returns one declared release cell; linkage and min_runtime are declared, not measured."""
    windows = os == "windows"
    machine = "aarch64" if arch == "arm64" else arch
    return {
        "arch": arch,
        "constraints": ["@platforms//os:" + os, "@platforms//cpu:" + machine],
        "exe": "dx.exe" if windows else "dx",
        "format": "zip" if windows else "tar.gz",
        "linkage": "dynamic",
        "machine": machine,
        "min_runtime": min_runtime,
        "name": "dx-" + os + "-" + arch,
        "os": os,
        "runtime": [],
    }

RELEASE_MATRIX = [
    _cell("linux", "x86_64", "glibc-2.28"),
    _cell("linux", "arm64", "glibc-2.28"),
    _cell("macos", "arm64", "macos-14.0"),
    _cell("windows", "x86_64", "windows-10"),
    _cell("windows", "arm64", "windows-10"),
]

def release_matrix_names():
    """Returns the ordered release cell names."""
    return [cell["name"] for cell in RELEASE_MATRIX]

def release_matrix_cell(name):
    """Returns one release cell by name, or None."""
    for cell in RELEASE_MATRIX:
        if cell["name"] == name:
            return cell
    return None

def release_matrix_error(name):
    """Validates one matrix cell name."""
    if release_matrix_cell(name) == None:
        return ("release_matrix: unknown artifact '" + str(name) +
                "': want one of " + ", ".join(release_matrix_names()))
    return ""

def release_cell_archive(cell):
    """Returns the archive file name of one release cell."""
    return archive_filenames(cell["name"], cell["format"])[0]

def release_matrix_json():
    """Returns the release-matrix/v1 document for every cell."""
    cells = [
        {
            "arch": cell["arch"],
            "exe": cell["exe"],
            "format": cell["format"],
            "machine": cell["machine"],
            "name": cell["name"],
            "os": cell["os"],
            "runtime": [Label(runtime).name for runtime in cell["runtime"]],
        }
        for cell in RELEASE_MATRIX
    ]
    return json.encode_indent({"cells": cells, "schema": RELEASE_MATRIX_SCHEMA}, indent = "  ") + "\n"

def _release_matrix_file_impl(ctx):
    """Writes release_matrix.json."""
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, release_matrix_json())
    return [DefaultInfo(files = depset([out]))]

release_matrix_file = rule(
    implementation = _release_matrix_file_impl,
    doc = "Writes the release-matrix/v1 JSON for every release cell.",
)

def release_cells(name):
    """Declares one archive per release cell plus the `name` alias to the host cell archive."""
    for cell in RELEASE_MATRIX:
        cell_name = cell["name"]
        constraints = cell["constraints"]
        shipped_inventory(
            name = cell_name + "_inventory",
            app = "//cli/cli:dx",
            target_compatible_with = constraints,
        )
        notice_bundle(
            name = cell_name + "_notice",
            inventory = ":" + cell_name + "_inventory",
            root = "//deploy/release:" + cell_name,
            texts = [":" + cell_name + "_inventory_texts"],
            target_compatible_with = constraints,
        )
        consumer_archive(
            name = cell_name,
            app = "//cli/cli:dx",
            arch = cell["machine"],
            format = cell["format"],
            linkage = cell["linkage"],
            min_runtime = cell["min_runtime"],
            notice = ":" + cell_name + "_notice_notice",
            os = cell["os"],
            runtime = cell["runtime"],
            target_compatible_with = constraints,
        )
        native.config_setting(
            name = cell_name + "_host",
            constraint_values = constraints,
        )
    native.filegroup(
        name = name + "_unsupported",
        target_compatible_with = ["@platforms//:incompatible"],
    )
    native.alias(
        name = name,
        actual = select(dict(
            [(":" + cell["name"] + "_host", ":" + cell["name"] + "_archive") for cell in RELEASE_MATRIX],
            **{"//conditions:default": ":" + name + "_unsupported"}
        )),
    )
