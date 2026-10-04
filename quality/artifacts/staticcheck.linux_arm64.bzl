"""staticcheck standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "staticcheck",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE-THIRD-PARTY",
                "size": 6442,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "staticcheck/staticcheck",
                "size": 15713197,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE",
                "size": 1058,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "staticcheck/staticcheck",
    "executable_sha256": "8c1eb39fac43f5b01b73a9404bb0ebb740127f62e660f266f5ab8b43e1bf7d86",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/dominikh/go-tools/blob/2026.2.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "594421f28ba620ea14b98377cb84a309cf73cc420ba0f52f30c7fa4d92fd2b0e",
    "size": 8538312,
    "tool": "staticcheck",
    "upstream_version": "2026.2.1",
    "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_linux_arm64.tar.gz",
}
