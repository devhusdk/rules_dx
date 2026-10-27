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

def cell_cases(package, cells):
    """Returns the select cases that accept only the given cells.

    Args:
        package: Package that declares the settings.
        cells: Cell keys to accept.

    Returns:
        Select cases, with an incompatible default.
    """
    cases = {cell_setting(package, cell): [] for cell in sorted(cells)}
    cases["//conditions:default"] = [_INCOMPATIBLE]
    return cases

def cells_compatible_with(pinned, package, cells):
    """Returns a target_compatible_with select for the given cells.

    Args:
        pinned: Dict of pinned cells.
        package: Package that declares the settings.
        cells: Cell keys to accept.

    Returns:
        A select that fails analysis-compat for every other platform.
    """
    unknown = unknown_cells(pinned, cells)
    if unknown:
        fail("unknown cells: " + ", ".join(unknown))
    return select(cell_cases(package, cells))

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
