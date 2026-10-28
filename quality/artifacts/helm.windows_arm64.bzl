"""helm standalone artifact metadata (windows_arm64) -- GENERATED, do not edit."""

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
                "mode": "0o755",
                "name": "windows-arm64/",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "windows-arm64/helm.exe",
                "size": 62158336,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "windows-arm64/LICENSE",
                "size": 11373,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "windows-arm64/README.md",
                "size": 4410,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "windows-arm64/helm.exe",
    "executable_sha256": "82e20e023033d086f12b56fe8edaeb6f6ddba72e54152e28d103d259c795c0f1",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/helm/helm/blob/v4.3.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "373edaae86db70342ef39b014e2e7764ea8f9cff08714843daa5e55b5ee492f2",
    "size": 18086587,
    "tool": "helm",
    "upstream_version": "4.3.1",
    "url": "https://get.helm.sh/helm-v4.3.1-windows-arm64.zip",
}
