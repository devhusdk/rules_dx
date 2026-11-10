"""The powershell module extension."""

load("//core:repo.bzl", "pinned_archive")

_VERSION = "0.3.1"

def _powershell_impl(mctx):
    pinned_archive(
        name = "rules_powershell",
        sha256 = "8ef875aa1dad8dc0f8f568547d2283f3d4b686b8e50374d6cdda4c5688fe6b7f",
        urls = ["https://github.com/periareon/rules_powershell/releases/download/{0}/rules_powershell-{0}.tar.gz".format(_VERSION)],
    )
    return mctx.extension_metadata(reproducible = True)

powershell = module_extension(
    implementation = _powershell_impl,
    doc = "Creates the patched rules_powershell.",
)
