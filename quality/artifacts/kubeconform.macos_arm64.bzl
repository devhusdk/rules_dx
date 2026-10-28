"""kubeconform standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

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
                "size": 11346,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "kubeconform",
                "size": 13356018,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "kubeconform",
    "executable_sha256": "858ceb584915c256262090195eda2579299149050c5c6a9788d0985076832313",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/yannh/kubeconform/blob/v0.8.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "f84f4dfbebf4a6b0b230385fa065a39ea35e02608c2b50d025dcf64775a69d67",
    "size": 7276844,
    "tool": "kubeconform",
    "upstream_version": "0.8.0",
    "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-darwin-arm64.tar.gz",
}
