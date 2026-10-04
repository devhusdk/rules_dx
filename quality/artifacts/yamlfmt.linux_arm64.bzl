"""yamlfmt standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
                "mode": "0o644",
                "name": "LICENSE",
                "size": 11357,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 4824,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "yamlfmt",
                "size": 4522168,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "yamlfmt",
    "executable_sha256": "3c38815a616836c6b1e4586d296e5164ddd8b5385ee3c4874bc52f6b8a9ce652",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/google/yamlfmt/blob/main/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "5b2689c963b177271330c5ce8ca7396751107e5a826be46f03d2cb9b6f0c7784",
    "size": 1779507,
    "tool": "yamlfmt",
    "upstream_version": "v0.21.0",
    "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Linux_arm64.tar.gz",
}
