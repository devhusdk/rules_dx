"""The PHP cells and what each runtime can load."""

def _cell(os_name, cpu, interpreter = "php", pcov = False):
    return struct(
        exec_compatible_with = ["@platforms//os:" + os_name, "@platforms//cpu:" + cpu],
        interpreter = interpreter,
        pcov = pcov,
    )

PHP_CELLS = {
    "linux_x86_64": _cell("linux", "x86_64", pcov = True),
    "linux_arm64": _cell("linux", "aarch64", pcov = True),
    "macos_arm64": _cell("macos", "aarch64", pcov = True),
    "windows_x86_64": _cell("windows", "x86_64", interpreter = "php.exe", pcov = True),
}

def unsupported_value(name):
    """Declares the unsupported constraint_value and its setting."""
    native.constraint_setting(name = name + "_setting")
    native.constraint_value(
        name = name,
        constraint_setting = ":" + name + "_setting",
        visibility = ["//visibility:public"],
    )

def cell_settings(name, cells):
    """Declares one config_setting per cell, named name_<cell>."""
    for cell in sorted(cells.keys()):
        native.config_setting(
            name = name + "_" + cell,
            constraint_values = cells[cell].exec_compatible_with,
        )
