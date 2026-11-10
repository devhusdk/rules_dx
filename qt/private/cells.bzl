"""Qualified Qt cells as target compatibility and consumer archive settings."""

load("//core:cells.bzl", "unsupported_constraint")
load("//core:platform_support.bzl", "compatible_with")
load("//qt/private/sdk:pins.bzl", "CELLS", "IDENTITIES")
load("//qt/private/sdk:repos.bzl", "PLATFORM_SUPPORT", "qualified_cells")

def qt_consumer_identity(cell):
    """Returns the consumer_archive arch, os and min_runtime of one qualified Qt cell."""
    return IDENTITIES[cell]

_CONSUMER_CELLS = [cell for cell in qualified_cells() if IDENTITIES[cell] != None]

_BUNDLE_CELLS = [cell for cell in _CONSUMER_CELLS if CELLS[cell].needs != None]

def _cell_setting(cell):
    return Label("//qt/private/platforms:cell_" + cell)

QT_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

QT_BUNDLE_COMPATIBLE_WITH = select(
    {_cell_setting(cell): [] for cell in _BUNDLE_CELLS} |
    {"//conditions:default": [Label("@platforms//:incompatible"), unsupported_constraint("//qt/private")]},
)

QT_CONSUMER_MIN_RUNTIME = select(
    {_cell_setting(cell): qt_consumer_identity(cell).min_runtime for cell in _CONSUMER_CELLS} |
    {"//conditions:default": ""},
)

_RUNTIME_CELLS = [cell for cell in _BUNDLE_CELLS if CELLS[cell].layout.os == "linux"]
_WINDOWS_CELLS = [cell for cell in _BUNDLE_CELLS if CELLS[cell].layout.os == "windows"]

QT_CONSUMER_RUNTIME = select(
    {_cell_setting(cell): [Label("//qt/private:os_runtime_" + cell)] for cell in _RUNTIME_CELLS} |
    {_cell_setting(cell): [Label("//cc/private/toolchains:msvc_vcruntime")] for cell in _WINDOWS_CELLS} |
    {"//conditions:default": []},
)
