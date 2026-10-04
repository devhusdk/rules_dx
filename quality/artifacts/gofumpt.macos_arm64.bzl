"""gofumpt standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

Regenerate with: bazel run //quality/artifacts:update
"""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "gofumpt_v0.12.0_darwin_arm64",
    "executable_sha256": "1a3e325b956ed12c21031d9e13382bd51a4fabb57de8d40428819b9583c3cfb9",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/mvdan/gofumpt/blob/v0.12.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "1a3e325b956ed12c21031d9e13382bd51a4fabb57de8d40428819b9583c3cfb9",
    "size": 3148210,
    "tool": "gofumpt",
    "upstream_version": "v0.12.0",
    "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_darwin_arm64",
}
