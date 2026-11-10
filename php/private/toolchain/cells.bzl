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
    "macos_arm64": _cell("macos", "aarch64"),
    "windows_x86_64": _cell("windows", "x86_64", interpreter = "php.exe"),
}
