"""Module extension for d/private/toolchain."""

load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "LDC_CELLS", "RULES_D_SHA256", "RULES_D_URLS", "RULES_D_VERSION", "TOOL_SOURCES")
load(":repos.bzl", "DROPPED_PATHS", "HUB_NAME", "ROOT_BUILD", "hub_build", "ldc_repo", "ldc_repository_name", "tool_build")


def tool_files(tool):
    """Returns the files written into one D quality tool source repository."""
    files = dict(TOOL_SOURCES[tool].files)
    files["BUILD.bazel"] = tool_build(tool)
    return files


def _d_impl(mctx):
    pinned_archive(
        name = "rules_d",
        dropped = DROPPED_PATHS,
        files = {"BUILD.bazel": ROOT_BUILD},
        patches = [
            Label("//patches/rules_d:pic_link.patch"),
        ],
        sha256 = RULES_D_SHA256,
        strip_prefix = "rules_d-" + RULES_D_VERSION,
        urls = RULES_D_URLS,
    )
    for cell in sorted(LDC_CELLS.keys()):
        ldc_repo(name = ldc_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    for tool in sorted(TOOL_SOURCES.keys()):
        source = TOOL_SOURCES[tool]
        pinned_archive(
            name = tool,
            dropped = source.dropped,
            files = tool_files(tool),
            sha256 = source.sha256,
            strip_prefix = source.strip_prefix,
            urls = source.urls,
        )
    return mctx.extension_metadata(reproducible = True)


d = module_extension(
    implementation = _d_impl,
    doc = "Creates rules_d, the pinned LDC compilers and the D toolchain hub.",
)
