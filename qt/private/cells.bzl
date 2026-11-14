"""Qualified Qt cells as target compatibility settings."""

load("//qt/private/toolchain:pins.bzl", "CELLS", "IDENTITIES")
load("//qt/private/toolchain:repos.bzl", "qualified_cells")

def qt_consumer_identity(cell):
    """Returns the consumer_archive arch, os and min_runtime of one qualified Qt cell."""
    return IDENTITIES[cell]

_CONSUMER_CELLS = [cell for cell in qualified_cells() if IDENTITIES[cell] != None]

_BUNDLE_CELLS = [cell for cell in _CONSUMER_CELLS if CELLS[cell].needs != None]

def _cell_setting(cell):
    return Label("//qt/private/platforms:cell_" + cell)

_INCOMPATIBLE = Label("@platforms//:incompatible")

_UNSUPPORTED = Label("//qt/private:unsupported")

QT_COMPATIBLE_WITH = select(
    {setting: [] for setting in [_cell_setting(cell) for cell in sorted(CELLS)]} |
    {"//conditions:default": [_INCOMPATIBLE, _UNSUPPORTED]},
)

QT_BUNDLE_COMPATIBLE_WITH = select(
    {_cell_setting(cell): [] for cell in _BUNDLE_CELLS} |
    {"//conditions:default": [_INCOMPATIBLE, _UNSUPPORTED]},
)
