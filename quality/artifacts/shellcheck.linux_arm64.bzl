"""shellcheck standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
                "name": "shellcheck-v0.11.0/LICENSE.txt",
                "size": 35149,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "shellcheck-v0.11.0/README.txt",
                "size": 2374,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "shellcheck-v0.11.0/shellcheck",
                "size": 55043352,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "shellcheck-v0.11.0/shellcheck",
    "executable_sha256": "127f13925eadd52c341bca0ebaf9ab0dbd78c6468f30a8f262a528bf8de47546",
    "interpreter": None,
    "licenses": [
        {
            "name": "GPL-3.0",
            "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "68a8133197a50beb8803f8d42f9908d1af1c5540d4bb05fdfca8c1fa47decefc",
    "size": 11668131,
    "tool": "shellcheck",
    "upstream_version": "v0.11.0",
    "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.aarch64.tar.gz",
}
