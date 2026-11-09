"""PowerShell toolchain hub tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load("//libs/starlark:toolchain.bzl", "cell_repo", "hub_checks", "pins_hub")

def _checks():
    cells = PINS["dx_powershell_toolchains"]["cells"].keys()
    toolchain_type = str(Label("@rules_powershell//powershell:toolchain_type"))
    hub = pins_hub("dx_powershell_toolchains", cells, toolchain_type)
    return hub_checks(hub, "//powershell:toolchain_type", len(cells), ["@rules_powershell"]) + [
        expect_equal("cells are the six host cells", len(cells), 6),
        expect_contains("hub names each platform repo", hub, "@" + cell_repo("dx_powershell_toolchains", "linux_x86_64") + "//:toolchain"),
    ]

def powershell_metadata_tests(name):
    """Declares the PowerShell toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
