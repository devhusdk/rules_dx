"""shfmt standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "shfmt_v3.14.1_windows_amd64.exe",
    "executable_sha256": "13629ce28442ca80b6b5a819f7574ab39e1c28c6e26734ca816c9714e04851df",
    "interpreter": None,
    "licenses": [
        {
            "name": "BSD-3-Clause",
            "source": "https://github.com/mvdan/sh/blob/v3.14.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "13629ce28442ca80b6b5a819f7574ab39e1c28c6e26734ca816c9714e04851df",
    "size": 3801600,
    "tool": "shfmt",
    "upstream_version": "v3.14.1",
    "url": "https://github.com/mvdan/sh/releases/download/v3.14.1/shfmt_v3.14.1_windows_amd64.exe",
}
