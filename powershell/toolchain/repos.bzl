"""Hermetic PowerShell acquisition and toolchain registration."""

load("//libs/starlark:toolchain.bzl", "toolchain_target")
load(
    ":pins.bzl",
    "PWSH_CELLS",
    "PWSH_DOWNLOAD",
)

HUB_NAME = "dx_powershell_toolchains"

_PWSH_BUILD = """load("@rules_powershell//powershell:pwsh_toolchain.bzl", "pwsh_toolchain")

filegroup(
    name = "powershell_bin",
    srcs = ["{binary}"],
    data = glob(
        include = ["**"],
        exclude = ["BUILD.bazel"],
    ),
)

pwsh_toolchain(
    name = "toolchain",
    pwsh = ":powershell_bin",
    visibility = ["//visibility:public"],
)
"""

def repo_name(cell):
    """Returns the repository name of one PowerShell build."""
    return HUB_NAME + "__" + cell

def _pwsh_impl(rctx):
    cell = PWSH_CELLS[rctx.attr.cell]
    rctx.download_and_extract(url = [PWSH_DOWNLOAD + cell.artifact], sha256 = cell.sha256)
    if not cell.binary.endswith(".exe"):
        rctx.file(cell.binary, rctx.read(cell.binary), executable = True)
    rctx.file("BUILD.bazel", _PWSH_BUILD.format(binary = cell.binary))
    return rctx.repo_metadata(reproducible = True)

pwsh_repo = repository_rule(
    implementation = _pwsh_impl,
    doc = "Downloads the pinned PowerShell build of one platform.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Platform key in PWSH_CELLS."),
    },
)

def hub_build():
    """Returns the hub BUILD file: one toolchain per platform."""
    return "\n".join([
        toolchain_target(
            name = repo_name(cell),
            target_compatible_with = PWSH_CELLS[cell].exec_compatible_with,
            toolchain = "@{}//:toolchain".format(repo_name(cell)),
            toolchain_type = str(Label("@rules_powershell//powershell:toolchain_type")),
            visibility = ["//visibility:public"],
        )
        for cell in sorted(PWSH_CELLS.keys())
    ])
