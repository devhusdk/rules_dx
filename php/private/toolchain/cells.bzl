"""The PHP cells and the interpreter of each runtime."""

def _cell(os_name, cpu, interpreter = "php"):
    return struct(
        exec_compatible_with = ["@platforms//os:" + os_name, "@platforms//cpu:" + cpu],
        interpreter = interpreter,
    )

PHP_CELLS = {
    "linux_x86_64": _cell("linux", "x86_64"),
    "linux_arm64": _cell("linux", "aarch64"),
    "macos_arm64": _cell("macos", "aarch64"),
    "windows_x86_64": _cell("windows", "x86_64", interpreter = "php.exe"),
}
