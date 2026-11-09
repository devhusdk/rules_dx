"""PowerShell toolchain pin and hub tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "PWSH_CELLS", "PWSH_VERSION", "RULES_POWERSHELL_SHA256")
load(":repos.bzl", "hub_build", "repo_name")

def _checks():
    hub = hub_build()
    return [
        expect_true("pwsh digests are sha256", all([len(c.sha256) == 64 for c in PWSH_CELLS.values()])),
        expect_true("artifacts name the version", all([PWSH_VERSION in c.artifact for c in PWSH_CELLS.values()])),
        expect_equal("rules_powershell digest", len(RULES_POWERSHELL_SHA256), 64),
        expect_true("hub loads nothing", "load(" not in hub),
        expect_true("hub names no rules_powershell label", "@rules_powershell" not in hub),
        expect_contains("hub uses the rules_dx type", hub, "//powershell/toolchain/type:toolchain_type"),
        expect_equal("one toolchain per platform", hub.count("toolchain(\n"), len(PWSH_CELLS)),
        expect_contains("hub names each platform repo", hub, "@" + repo_name("linux_x64") + "//:toolchain"),
    ]

def powershell_metadata_tests(name):
    """Declares the PowerShell toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
