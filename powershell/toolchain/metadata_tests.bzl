"""PowerShell toolchain pin and hub tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//libs/starlark:toolchain.bzl", "hub_checks")
load(":pins.bzl", "PWSH_CELLS", "PWSH_VERSION", "RULES_POWERSHELL_SHA256")
load(":repos.bzl", "hub_build", "repo_name")

def _checks():
    hub = hub_build()
    return hub_checks(hub, "//powershell:toolchain_type", len(PWSH_CELLS), ["@rules_powershell"]) + [
        expect_true("pwsh digests are sha256", all([len(c.sha256) == 64 for c in PWSH_CELLS.values()])),
        expect_true("artifacts name the version", all([PWSH_VERSION in c.artifact for c in PWSH_CELLS.values()])),
        expect_equal("rules_powershell digest", len(RULES_POWERSHELL_SHA256), 64),
        expect_contains("hub names each platform repo", hub, "@" + repo_name("linux_x86_64") + "//:toolchain"),
    ]

def powershell_metadata_tests(name):
    """Declares the PowerShell toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
