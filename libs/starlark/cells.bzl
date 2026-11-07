"""Per-cell platform settings and compatibility selects, shared by toolchains."""

_INCOMPATIBLE = Label("@platforms//:incompatible")

def cell_setting(package, cell):
    """Returns the config_setting label that matches one cell."""
    return Label(package + ":cell_" + cell)

def unknown_cells(pinned, cells):
    """Returns the cells that are not pinned."""
    return [cell for cell in cells if cell not in pinned]

def unsupported_constraint(package):
    """Returns the constraint_value that no platform sets, marking a language as unsupported."""
    return Label(package + ":unsupported")

def unsupported_value(name):
    """Declares the unsupported constraint_value and its setting."""
    native.constraint_setting(name = name + "_setting")
    native.constraint_value(
        name = name,
        constraint_setting = ":" + name + "_setting",
        visibility = ["//visibility:public"],
    )

def cell_cases(package, cells, unsupported = False, settings = {}):
    """Returns the select cases that accept only the given cells."""
    cases = {settings.get(cell, cell_setting(package, cell)): [] for cell in sorted(cells)}
    cases["//conditions:default"] = [_INCOMPATIBLE]
    if unsupported:
        cases["//conditions:default"].append(unsupported_constraint(package))
    return cases

def cells_compatible_with(pinned, package, cells, unsupported = False, settings = {}):
    """Returns a target_compatible_with select for the given cells."""
    unknown = unknown_cells(pinned, cells)
    if unknown:
        fail("unknown cells: " + ", ".join(unknown))
    return select(cell_cases(package, cells, unsupported, settings))

def cell_settings(name, cells):
    """Declares one config_setting per cell, named name_<cell>."""
    for cell in sorted(cells.keys()):
        native.config_setting(
            name = name + "_" + cell,
            constraint_values = cells[cell].exec_compatible_with,
        )
