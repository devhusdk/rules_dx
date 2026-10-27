"""Per-cell OTP platform settings and compatibility selects."""

load(":pins.bzl", "CELLS")

_INCOMPATIBLE = Label("@platforms//:incompatible")

def cell_setting(cell):
    """Returns the config_setting label that matches one OTP cell."""
    return Label("//erlang/toolchain:cell_" + cell)

def unknown_cells(cells):
    """Returns the cells that are not pinned OTP cells."""
    return [cell for cell in cells if cell not in CELLS]

def cell_cases(cells):
    """Returns the select cases that accept only the given OTP cells."""
    cases = {cell_setting(cell): [] for cell in sorted(cells)}
    cases["//conditions:default"] = [_INCOMPATIBLE]
    return cases

def otp_compatible_with(cells):
    """Returns a target_compatible_with select for the given OTP cells."""
    unknown = unknown_cells(cells)
    if unknown:
        fail("unknown OTP cells: " + ", ".join(unknown))
    return select(cell_cases(cells))

def otp_cell_settings(name):
    """Declares one config_setting per pinned OTP cell, named after the prefix."""
    for cell in sorted(CELLS.keys()):
        native.config_setting(
            name = name + "_" + cell,
            constraint_values = CELLS[cell].exec_compatible_with,
        )
