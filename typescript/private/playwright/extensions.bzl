"""Module extension for typescript/private/playwright."""

load("//core:repo.bzl", "hub_repo")
load(":repos.bzl", "chromium_repo", "hub_build", "repository_name")

visibility(["//..."])

def _playwright_impl(module_ctx):
    pins = json.decode(module_ctx.read(Label("//typescript:pins.json")))
    for platform in sorted(pins["cells"].keys()):
        cell = pins["cells"][platform]
        chromium_repo(
            name = repository_name(platform),
            platform = platform,
            revision = pins["revision"],
            sha256 = cell["sha256"],
            urls = cell["urls"],
        )
    hub_repo(name = "dx_playwright_chromium", files = {"BUILD.bazel": hub_build(pins["cells"].keys())})
    return module_ctx.extension_metadata(reproducible = True)

playwright = module_extension(
    implementation = _playwright_impl,
)
