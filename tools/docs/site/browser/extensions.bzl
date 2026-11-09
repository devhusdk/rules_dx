"""Module extension for tools/docs/site/browser."""

load("//libs/starlark:repo.bzl", "hub_repo")
load(":repos.bzl", "ARTIFACTS", "artifact_errors", "firefox_repo", "hub_build", "repository_name", "url_for")


_HUB_NAME = "dx_firefox"


def _firefox_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("firefox: the extension is only reachable from a module graph")
    errors = artifact_errors()
    if len(errors) > 0:
        fail("firefox: " + "; ".join(errors))
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        firefox_repo(
            name = repository_name(platform),
            archive_format = artifact.archive_format,
            asset = artifact.asset,
            executable = artifact.executable,
            sha256 = artifact.sha256,
            url = url_for(artifact),
        )
    hub_repo(name = _HUB_NAME, files = {"BUILD.bazel": hub_build()})
    return module_ctx.extension_metadata(reproducible = True)


firefox = module_extension(
    implementation = _firefox_impl,
)
