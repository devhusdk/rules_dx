"""Pins the qualified Python runtime ownership and patch retention."""

load("//libs/starlark:defs.bzl", "starlark_test")

def python_runtime_ownership_tests(name):
    """Instantiates the Python runtime ownership retention tests."""
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            "//:MODULE.bazel": "\"//patches:aspect_rules_py_site_packages.patch\",\n\"//patches:aspect_rules_py_venv_interpreter.patch\",",
            "//patches:aspect_rules_py_site_packages.patch": "\"{}/Lib/{}/site-packages\" if is_windows",
            "//patches:aspect_rules_py_venv_interpreter.patch": "bin_python_name = \"python.exe\" if is_windows else \"python\"",
        },
    )
