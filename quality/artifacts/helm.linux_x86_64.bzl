"""helm standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

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
                "name": "linux-amd64",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "linux-amd64/helm",
                "size": 66314400,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "linux-amd64/LICENSE",
                "size": 11373,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "linux-amd64/README.md",
                "size": 4410,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "linux-amd64/helm",
    "executable_sha256": "b66cb19f2b31f9d427632ea6c4b4a1ff89b74ecaeaee323cd166da41a7af53bd",
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
    "sha256": "0403a88ae20913d11b10a2467b50087721fea7581736da6b9d3992f5357fe54b",
    "size": 20368240,
    "tool": "helm",
    "upstream_version": "4.3.1",
    "url": "https://get.helm.sh/helm-v4.3.1-linux-amd64.tar.gz",
}
