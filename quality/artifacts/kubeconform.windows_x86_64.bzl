"""kubeconform standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "size": 14132224,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "kubeconform.exe",
    "executable_sha256": "642bf8ea0d614a8c75d2bd012eb6042a90589ff846c69f04863afa446b30f2c1",
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
    "sha256": "e3f56102bcf4f50b034a567e2482a1c5330799983ddd655952310211aef73d93",
    "size": 7615744,
    "tool": "kubeconform",
    "upstream_version": "0.8.0",
    "url": "https://github.com/yannh/kubeconform/releases/download/v0.8.0/kubeconform-windows-amd64.zip",
}
