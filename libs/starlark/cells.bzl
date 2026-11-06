"""Per-cell platform settings and compatibility selects, shared by toolchains."""

_INCOMPATIBLE = Label("@platforms//:incompatible")

def cell_setting(package, cell):
    """Returns the config_setting label that matches one cell.

    Args:
        package: Package that declares the settings, such as "//haskell/toolchain".
        cell: Cell key.

    Returns:
        The config_setting label.
    """
    return Label(package + ":cell_" + cell)

def unknown_cells(pinned, cells):
    """Returns the cells that are not pinned.

    Args:
        pinned: Dict of pinned cells.
        cells: Cell keys to check.

    Returns:
        The cell keys missing from pinned.
    """
    return [cell for cell in cells if cell not in pinned]

def unsupported_constraint(package):
    """Returns the constraint_value that no platform sets, marking a language as unsupported.

    Args:
        package: Package that declares the settings.

    Returns:
        The constraint_value label.
    """
    return Label(package + ":unsupported")

def unsupported_value(name):
    """Declares the unsupported constraint_value and its setting.

    Args:
        name: Name of the constraint_value, normally "unsupported".
    """
    native.constraint_setting(name = name + "_setting")
    native.constraint_value(
        name = name,
        constraint_setting = ":" + name + "_setting",
        visibility = ["//visibility:public"],
    )

def cell_cases(package, cells, unsupported = False, settings = {}):
    """Returns the select cases that accept only the given cells.

    Args:
        package: Package that declares the settings.
        cells: Cell keys to accept.
        unsupported: Also require the package's unsupported constraint in the default, so dx can name the language.
        settings: Maps a cell to the setting to use instead of the package's cell setting.

    Returns:
        Select cases, with an incompatible default.
    """
    cases = {settings.get(cell, cell_setting(package, cell)): [] for cell in sorted(cells)}
    cases["//conditions:default"] = [_INCOMPATIBLE]
    if unsupported:
        cases["//conditions:default"].append(unsupported_constraint(package))
    return cases

def cells_compatible_with(pinned, package, cells, unsupported = False, settings = {}):
    """Returns a target_compatible_with select for the given cells.

    Args:
        pinned: Dict of pinned cells.
        package: Package that declares the settings.
        cells: Cell keys to accept.
        unsupported: Also require the package's unsupported constraint on other platforms.
        settings: Maps a cell to the setting to use instead of the package's cell setting.

    Returns:
        A select that fails analysis-compat for every other platform.
    """
    unknown = unknown_cells(pinned, cells)
    if unknown:
        fail("unknown cells: " + ", ".join(unknown))
    return select(cell_cases(package, cells, unsupported, settings))

def cell_settings(name, cells):
    """Declares one config_setting per cell, named name_<cell>.

    Args:
        name: Prefix for the setting names.
        cells: Dict of cells with exec_compatible_with.
    """
    for cell in sorted(cells.keys()):
        native.config_setting(
            name = name + "_" + cell,
            constraint_values = cells[cell].exec_compatible_with,
        )
