"""shellcheck standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
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
                    "size": 55043352,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "shellcheck-v0.11.0/shellcheck",
        "executable_sha256": "127f13925eadd52c341bca0ebaf9ab0dbd78c6468f30a8f262a528bf8de47546",
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
        "sha256": "68a8133197a50beb8803f8d42f9908d1af1c5540d4bb05fdfca8c1fa47decefc",
        "size": 11668131,
        "tool": "shellcheck",
        "upstream_version": "0.11.0",
        "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.aarch64.tar.gz",
    },
    "linux_x86_64": {
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
        "sha256": "b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6",
        "size": 3773312,
        "tool": "shellcheck",
        "upstream_version": "0.11.0",
        "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.gz",
    },
    "macos_arm64": {
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
                    "size": 61551911,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "shellcheck-v0.11.0/shellcheck",
        "executable_sha256": "61c17246d69f012cd458ae82f244c46023dac75d1b69733ca1cc7d28fb270fd7",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-3.0",
                "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "339b930feb1ea764467013cc1f72d09cd6b869ebf1013296ba9055ab2ffbd26f",
        "size": 11370575,
        "tool": "shellcheck",
        "upstream_version": "0.11.0",
        "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.darwin.aarch64.tar.gz",
    },
    "windows_x86_64": {
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
                    "name": "LICENSE.txt",
                    "size": 35149,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "README.txt",
                    "size": 2374,
                },
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "shellcheck.exe",
                    "size": 34778624,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "shellcheck.exe",
        "executable_sha256": "c9e82ada36ef4b8d4caf1f97fa89289048c8f4a33c2c76ffffc88bfe09ff00c5",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-3.0",
                "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "8a4e35ab0b331c85d73567b12f2a444df187f483e5079ceffa6bda1faa2e740e",
        "size": 8068167,
        "tool": "shellcheck",
        "upstream_version": "0.11.0",
        "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.zip",
    },
}
