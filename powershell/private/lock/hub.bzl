"""The powershell hub: one repository whose packages are the locked Gallery modules."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_PACKAGING = ["_rels/**", "package/**", "[Content_Types].xml", "%5BContent_Types%5D.xml", ".signature.p7s"]

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    return "\n".join([
        'load("@rules_powershell//powershell:defs.bzl", "pwsh_library")',
        "",
        "pwsh_library(",
        '    name = "%s",' % name,
        '    srcs = ["%s.psd1", "%s.psm1"],' % (name, name),
        "    data = glob(",
        '        ["**"],',
        "        exclude = %s," % json.encode(["BUILD.bazel", name + ".psd1", name + ".psm1"] + _PACKAGING),
        "    ),",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _powershell_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

powershell = module_extension(
    implementation = _powershell_impl,
    tag_classes = {"lock": lock_tag},
)
