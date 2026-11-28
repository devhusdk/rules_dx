"""PowerShell toolchain hub tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//core:toolchain.bzl", "cell_repo", "hub_checks", "pins_hub")
load("//core/private/testing:defs.bzl", "expect_contains", "expect_equal", "starlark_test")

def _checks():
    cells = PINS["dx_powershell_toolchains"]["cells"].keys()
    toolchain_type = str(Label("//powershell:toolchain_type"))
    hub = pins_hub("dx_powershell_toolchains", cells, toolchain_type, "target")
    upstream = ["@rules_powershell//", "@@{}//".format(Label("@rules_powershell").repo_name)]
    return hub_checks(hub, 'toolchain_type = "{}"'.format(toolchain_type), len(cells), upstream) + [
        expect_equal("cells are the six host cells", len(cells), 6),
        expect_contains("hub names each platform repo", hub, "@" + cell_repo("dx_powershell_toolchains", "linux_x86_64") + "//:toolchain"),
    ]

def powershell_metadata_tests(name, size):
    """Declares the PowerShell toolchain metadata test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
