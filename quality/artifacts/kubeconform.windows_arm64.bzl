"""kubeconform standalone artifact metadata (windows_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
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
                "name": "kubeconform.exe",
                "size": 12994560,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "kubeconform.exe",
    "executable_sha256": "604f33351f2f476f4311ed9a4ac274ab9fa4d0087ee1158c1440337e489b8b4e",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/yannh/kubeconform/blob/v0.8.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "4f3c9889f5f3a1e4aba84f9212f599ad3164d1fb32175fba3a53b505b0fffd0f",
    "size": 6931379,
    "tool": "kubeconform",
    "upstream_version": "0.8.0",
    "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-windows-arm64.zip",
}
