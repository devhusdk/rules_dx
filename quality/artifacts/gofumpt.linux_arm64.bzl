"""gofumpt standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
    "executable": "gofumpt_v0.12.0_linux_arm64",
    "executable_sha256": "4322f081241ab0618356aa5921a8c110e4c7acfc17175cc15dc57b2f4a4b652d",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/mvdan/gofumpt/blob/v0.12.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "4322f081241ab0618356aa5921a8c110e4c7acfc17175cc15dc57b2f4a4b652d",
    "size": 3211424,
    "tool": "gofumpt",
    "upstream_version": "v0.12.0",
    "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_linux_arm64",
}
