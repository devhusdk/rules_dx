"""shellcheck standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

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
                "name": "shellcheck-v0.11.0/LICENSE.txt",
                "size": 35149,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "shellcheck-v0.11.0/README.txt",
                "size": 2374,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "shellcheck-v0.11.0/shellcheck",
                "size": 16213136,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "shellcheck-v0.11.0/shellcheck",
    "executable_sha256": "4da528ddb3a4d1b7b24a59d4e16eb2f5fd960f4bd9a3708a15baddbdf1d5a55b",
    "interpreter": None,
    "licenses": [
        {
            "name": "GPL-3.0",
            "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6",
    "size": 3773312,
    "tool": "shellcheck",
    "upstream_version": "0.11.0",
    "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.gz",
}
