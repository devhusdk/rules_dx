"""kubeconform standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "size": 12953083,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "kubeconform",
    "executable_sha256": "7e77b104b3ae696389f91971c60fd58c72f1f3dc218f139df67a1b070959c012",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/yannh/kubeconform/blob/v0.8.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "1f53fc8e81258197a35e8603054162a5af1de8c5af13746c71ab680d9534ed87",
    "size": 6901136,
    "tool": "kubeconform",
    "upstream_version": "0.8.0",
    "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-linux-arm64.tar.gz",
}
