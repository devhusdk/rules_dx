"""Module extension for tools/testing."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")


_PLATFORMS = ["darwin-arm64", "darwin-x86_64", "linux-arm64", "linux-x86_64", "windows-x86_64"]


_RELEASE = "https://github.com/bazelbuild/bazel/releases/download/{v}/bazel-{v}-{p}{s}"


def _suffix(platform):
    return ".exe" if platform.startswith("windows-") else ""


def _fetch_shas(mctx, version):
    shas = {}
    for platform in _PLATFORMS:
        path = "sha256_" + platform
        mctx.download(
            url = _RELEASE.format(v = version, p = platform, s = _suffix(platform)) + ".sha256",
            output = path,
        )
        sha256 = mctx.read(path).strip().split(" ")[0]
        valid = len(sha256) == 64 and all([c in "0123456789abcdef" for c in sha256.elems()])
        if not valid:
            fail("bad sha256 for bazel {} {}: {}".format(version, platform, sha256))
        shas[platform] = sha256
    return shas


def _nested_bazel_impl(mctx):
    """Declares one pinned Bazel binary per platform."""
    version = mctx.read(Label("//:.bazelversion")).strip()
    shas = mctx.facts.get(version)
    if not shas:
        shas = _fetch_shas(mctx, version)
    for platform in _PLATFORMS:
        http_file(
            name = "nested_bazel_" + platform.replace("-", "_"),
            downloaded_file_path = "bazel" + _suffix(platform),
            executable = True,
            sha256 = shas[platform],
            urls = [_RELEASE.format(v = version, p = platform, s = _suffix(platform))],
        )
    return mctx.extension_metadata(reproducible = True, facts = {version: shas})


nested_bazel = module_extension(
    implementation = _nested_bazel_impl,
    doc = "Creates the pinned Bazel binaries for nested Bazel tests.",
)
