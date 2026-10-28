"""helm standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "name": "linux-arm64",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "linux-arm64/helm",
                "size": 61538464,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "linux-arm64/LICENSE",
                "size": 11373,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "linux-arm64/README.md",
                "size": 4410,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "linux-arm64/helm",
    "executable_sha256": "d459cbff1331af4326a6f97ea83c547742b4056dac7404f20fc436c084178c43",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/helm/helm/blob/v4.3.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "8cf8f66ecbc252ac8f7f06cca68aba88508ddfde3a6a34feb1678ad1694b4f88",
    "size": 17917453,
    "tool": "helm",
    "upstream_version": "4.3.1",
    "url": "https://get.helm.sh/helm-v4.3.1-linux-arm64.tar.gz",
}
