"""buildifier standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
    "executable": "buildifier-windows-arm64.exe",
    "executable_sha256": "55a276ad8b1ff46be48bf64e432264034ea69a45aa3914e89c1d1936f5c2d85c",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/bazel-contrib/buildtools/blob/v8.5.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "55a276ad8b1ff46be48bf64e432264034ea69a45aa3914e89c1d1936f5c2d85c",
    "size": 7451648,
    "tool": "buildifier",
    "upstream_version": "8.5.1",
    "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-windows-arm64.exe",
}
