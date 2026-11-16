"""Module extension for tools/docs/site/browser."""

load("//core:repo.bzl", "hub_repo")
load(":repos.bzl", "artifact_errors", "firefox_repo", "hub_build", "repository_name")

_HUB_NAME = "dx_firefox"

def _firefox_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("firefox: the extension is only reachable from a module graph")
    pins = json.decode(module_ctx.read(Label(":pins.json")))
    errors = artifact_errors(pins)
    if len(errors) > 0:
        fail("firefox: " + "; ".join(errors))
    for platform in sorted(pins["cells"].keys()):
        cell = pins["cells"][platform]
        firefox_repo(
            name = repository_name(platform),
            archive_format = cell["type"],
            sha256 = cell["sha256"],
            urls = cell["urls"],
        )
    hub_repo(name = _HUB_NAME, files = {"BUILD.bazel": hub_build(pins)})
    return module_ctx.extension_metadata(reproducible = True)

firefox = module_extension(
    implementation = _firefox_impl,
)
