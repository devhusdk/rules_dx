"""helm standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

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
                "name": "darwin-arm64",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "darwin-arm64/helm",
                "size": 63382738,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "darwin-arm64/LICENSE",
                "size": 11373,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "darwin-arm64/README.md",
                "size": 4410,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "darwin-arm64/helm",
    "executable_sha256": "c62da98a8e5728f5acc08b66bbe5c3ad1cb6b71d3d31b7bf939d9f4b6628c96d",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/helm/helm/blob/v4.3.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "6b4e61c8cd27d99e5a801e63ff25dd5c70b36875c2cd659d3e9ce6b76fc23198",
    "size": 18815398,
    "tool": "helm",
    "upstream_version": "4.3.1",
    "url": "https://get.helm.sh/helm-v4.3.1-darwin-arm64.tar.gz",
}
