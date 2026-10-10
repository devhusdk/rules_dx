"""staticcheck standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

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
                "name": "staticcheck",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE-THIRD-PARTY",
                "size": 6442,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE",
                "size": 1058,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "staticcheck/staticcheck",
                "size": 17050224,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "staticcheck/staticcheck",
    "executable_sha256": "ba8e1dd887ca0c6a60838fc880b023a137b1feac99bcc1566ba5533429c9b88e",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/dominikh/go-tools/blob/2026.2/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "df1fd2b42de9fdef42231329522ea8f1f6846a5eb1a0200fad14c90bba464d02",
    "size": 9419376,
    "tool": "staticcheck",
    "upstream_version": "2026.2",
    "url": "https://github.com/dominikh/go-tools/releases/download/2026.2/staticcheck_linux_amd64.tar.gz",
}
