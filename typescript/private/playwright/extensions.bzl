"""Module extension for typescript/private/playwright."""

load("//libs/starlark:repo.bzl", "hub_repo")
load(":repos.bzl", "ARTIFACTS", "chromium_repo", "hub_build", "repository_name")

visibility(["//..."])

CHROMIUM_VERSION = "156.0.8078.4"


_BASE_URL = "https://storage.googleapis.com/chrome-for-testing-public/" + CHROMIUM_VERSION


def _playwright_impl(module_ctx):
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        chromium_repo(
            name = repository_name(platform),
            sha256 = artifact.sha256,
            url = _BASE_URL + "/" + artifact.path,
        )
    hub_repo(name = "dx_playwright_chromium", files = {"BUILD.bazel": hub_build()})
    return module_ctx.extension_metadata(reproducible = True)


playwright = module_extension(
    implementation = _playwright_impl,
)
