"""yamlfmt standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "size": 4592978,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "yamlfmt",
    "executable_sha256": "20c76f7e84039db1a2059d23d50b13287de5d02913e4c977283b7027c8b1e5c3",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/google/yamlfmt/blob/main/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "4b417ecb94339d57e4c122ecc948c1a00fe328b5853266de9806e652a92858fa",
    "size": 1848266,
    "tool": "yamlfmt",
    "upstream_version": "v0.21.0",
    "url": "https://github.com/google/yamlfmt/releases/download/v0.21.0/yamlfmt_0.21.0_Darwin_arm64.tar.gz",
}
