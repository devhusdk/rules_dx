"""The powershell module extension."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "PWSH_CELLS", "RULES_POWERSHELL_SHA256", "RULES_POWERSHELL_URL")
load(":repos.bzl", "HUB_NAME", "hub_build", "pwsh_repo", "repo_name")

def _powershell_impl(mctx):
    pinned_archive(
        name = "rules_powershell",
        patches = [Label("//patches/rules_powershell:toolchain_type_alias.patch")],
        sha256 = RULES_POWERSHELL_SHA256,
        urls = [RULES_POWERSHELL_URL],
    )
    for cell in sorted(PWSH_CELLS.keys()):
        pwsh_repo(name = repo_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    return mctx.extension_metadata(reproducible = True)

powershell = module_extension(
    implementation = _powershell_impl,
    doc = "Creates rules_powershell, the pinned PowerShell builds and the toolchain hub.",
)
