"""jsonnet_lint standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "size": 11358,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 7699,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet",
                "size": 5898424,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnetfmt",
                "size": 3801272,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-lint",
                "size": 3932344,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-deps",
                "size": 3604664,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "jsonnet-lint",
    "executable_sha256": "e49bfced776c2f0bf73ca224e717fc33a5e519e0c8b5d57b9d735f34d1701fa8",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "3016b049254cb92548ba99304b6f44ec3795e6173f908e3ce3c5add57243fe20",
    "size": 6151063,
    "tool": "jsonnet_lint",
    "upstream_version": "0.22.0",
    "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_arm64.tar.gz",
}
