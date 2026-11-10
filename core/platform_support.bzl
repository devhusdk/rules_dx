"""One platform_support value per language, read by the support matrix and the language's compatibility select."""

load(":cells.bzl", "cell_setting", "unknown_cells", "unsupported_constraint")

_INCOMPATIBLE = Label("@platforms//:incompatible")

def platform_support(label, symbol, any_of, settings, package = None):
    """Returns the platforms a language's targets accept."""
    return struct(key = label + "%" + symbol, any_of = any_of, settings = settings, package = package)

def compatible_with(support):
    """Returns the target_compatible_with select of a platform_support."""
    default = [_INCOMPATIBLE]
    if support.package:
        default.append(unsupported_constraint(support.package))
    return select({setting: [] for setting in support.settings} | {"//conditions:default": default})

def cells_support(label, symbol, pinned, cells, package, field = "exec_compatible_with", unsupported = True, settings = {}):
    """Returns the platform_support of the given pinned cells."""
    unknown = unknown_cells(pinned, cells)
    if unknown:
        fail("unknown cells: " + ", ".join(unknown))
    return platform_support(
        label,
        symbol,
        [getattr(pinned[cell], field) for cell in cells],
        [settings.get(cell, cell_setting(package, cell)) for cell in sorted(cells)],
        package if unsupported else None,
    )

def os_cpu_support(label, symbol, platforms, settings, package):
    """Returns the platform_support of an {id: (os, cpu)} table matched by the given settings."""
    return platform_support(
        label,
        symbol,
        [["@platforms//os:" + os, "@platforms//cpu:" + cpu] for os, cpu in sorted(platforms.values())],
        settings,
        package,
    )
