"""Release matrix for the dx standalone archives: one cell per OS and CPU."""

load("//deploy/rules:archive.bzl", "archive_filenames", "consumer_archive")
load(":inventory.bzl", "shipped_inventory")
load(":sbom.bzl", "sbom_release")
load(":notice.bzl", "notice_bundle")

RELEASE_MATRIX_SCHEMA = "release-matrix/v1"

# Top-level archive members each runtime target ships. The Universal CRT is supplied by Windows.
RUNTIME_MEMBERS = {
    "//cc/toolchains:msvc_vcruntime": [
        "msvcp140.dll",
        "msvcp140_1.dll",
        "msvcp140_2.dll",
        "vcruntime140.dll",
        "vcruntime140_1.dll",
    ],
    "//cc/toolchains:windows_ucrt": [],
}

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
        "runtime": ["//cc/toolchains:msvc_vcruntime", "//cc/toolchains:windows_ucrt"] if windows else [],
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

def release_cell_symbols(cell):
    """Returns the debug symbols archive file name of one release cell, or None when it ships none."""
    if cell["os"] != "windows":
        return None
    return cell["name"] + "-pdb.zip"

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
            "runtime": [member for runtime in cell["runtime"] for member in RUNTIME_MEMBERS[runtime]],
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

def release_cells(name, cells = RELEASE_MATRIX, app = "//cli/cli:dx", pdb = "//cli/cli:dx_pdb"):
    """Declares one archive per release cell, a symbols zip per Windows cell, and the `name` aliases for the host cell."""
    for cell in cells:
        cell_name = cell["name"]
        constraints = cell["constraints"]
        shipped_inventory(
            name = cell_name + "_inventory",
            app = app,
            runtimes = cell["runtime"],
            target_compatible_with = constraints,
        )
        notice_bundle(
            name = cell_name + "_notice",
            inventory = ":" + cell_name + "_inventory",
            root = "//deploy/release:" + cell_name,
            runtimes = (":" + cell_name + "_inventory_runtimes") if cell["runtime"] else None,
            texts = [":" + cell_name + "_inventory_texts"],
            target_compatible_with = constraints,
        )
        consumer_archive(
            name = cell_name,
            app = app,
            arch = cell["machine"],
            format = cell["format"],
            license_file = "//:LICENSE",
            linkage = cell["linkage"],
            min_runtime = cell["min_runtime"],
            notice = ":" + cell_name + "_notice_notice",
            os = cell["os"],
            runtime = cell["runtime"],
            target_compatible_with = constraints,
        )
        sbom_release(
            name = cell_name + "_sbom",
            artifact = ":" + cell_name + "_archive",
            inventory = ":" + cell_name + "_inventory",
            runtimes = (":" + cell_name + "_inventory_runtimes") if cell["runtime"] else None,
            target_compatible_with = constraints,
        )
        symbols = release_cell_symbols(cell)
        if symbols:
            # The entry keeps the PDB basename that /pdbaltpath:%_PDB% writes into the exe.
            native.genrule(
                name = cell_name + "_pdb_archive",
                srcs = [pdb],
                outs = [symbols],
                cmd = "$(execpath @bazel_tools//tools/zip:zipper) c $@ $$(basename $(location " + pdb + "))=$(location " + pdb + ")",
                target_compatible_with = constraints,
                tools = ["@bazel_tools//tools/zip:zipper"],
            )
            native.genrule(
                name = cell_name + "_pdb_checksum",
                srcs = [":" + cell_name + "_pdb_archive"],
                outs = [symbols + ".sha256"],
                cmd = "$(location //deploy/rules:hasher) $(location :" + cell_name + "_pdb_archive) $(OUTS)",
                target_compatible_with = constraints,
                tools = ["//deploy/rules:hasher"],
            )
            sbom_release(
                name = cell_name + "_pdb_sbom",
                artifact = ":" + cell_name + "_pdb_archive",
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
            [(":" + cell["name"] + "_host", ":" + cell["name"] + "_archive") for cell in cells],
            **{"//conditions:default": ":" + name + "_unsupported"}
        )),
    )
    native.alias(
        name = name + "_sbom",
        actual = select(dict(
            [(":" + cell["name"] + "_host", ":" + cell["name"] + "_sbom_spdx") for cell in cells],
            **{"//conditions:default": ":" + name + "_unsupported"}
        )),
    )
    native.alias(
        name = name + "_pdb",
        actual = select(dict(
            [(":" + cell["name"] + "_host", ":" + cell["name"] + "_pdb_archive") for cell in cells if release_cell_symbols(cell)],
            **{"//conditions:default": ":" + name + "_unsupported"}
        )),
    )
    native.alias(
        name = name + "_pdb_sbom",
        actual = select(dict(
            [(":" + cell["name"] + "_host", ":" + cell["name"] + "_pdb_sbom_spdx") for cell in cells if release_cell_symbols(cell)],
            **{"//conditions:default": ":" + name + "_unsupported"}
        )),
    )
