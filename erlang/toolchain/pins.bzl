"""Pinned Erlang/OTP runtime, native closure and rules_erlang identities."""

OTP_VERSION = "29.1.2"

ERTS_VERSION = "17.1.1"

OTP_RELEASE = "29"

RULES_ERLANG_COMMIT = "13977b3309fd0b0d6795f322cffd86a8c161be64"

RULES_ERLANG_SHA256 = "830823cc562431cb61d7787bc716f3fa8101ef3afe57350c40f92379f262fde9"

RULES_ERLANG_URL = "https://github.com/bazelverse/rules_erlang/archive/" + RULES_ERLANG_COMMIT + ".tar.gz"

ERLFMT_VERSION = "1.8.0"

GETOPT_VERSION = "1.0.3"

TOOL_SOURCES = {
    "dx_erlfmt": struct(
        dropped = ["src/rebar3_fmt_prv.erl"],
        sha256 = "c7205f3a28dac589c623f21546d63d5b04cc669545d03485cb88552de0117f43",
        strip_prefix = "erlfmt-" + ERLFMT_VERSION,
        url = "https://github.com/WhatsApp/erlfmt/archive/refs/tags/v" + ERLFMT_VERSION + ".tar.gz",
    ),
    "dx_erlang_getopt": struct(
        dropped = ["ebin"],
        sha256 = "e5a4ec9c6312ec5a850dea13ffa2cd50850e694fd79597cf76acb548287134d9",
        strip_prefix = "getopt-" + GETOPT_VERSION,
        url = "https://github.com/jcomellas/getopt/archive/refs/tags/v" + GETOPT_VERSION + ".tar.gz",
    ),
}

GLIBC_FLOOR = "2.35"

RUNTIME_MIRROR = "https://launchpad.net/ubuntu/+archive/primary/+files"

CELLS = {
    "linux_amd64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        otp_sha256 = "1df9e6285b4d14319a789176db60c273131abebdfa6b5474a084dba49b0e54a2",
        otp_url = "https://builds.hex.pm/builds/otp/amd64/ubuntu-22.04/OTP-" + OTP_VERSION + ".tar.gz",
        strip_prefix = "OTP-" + OTP_VERSION,
        runtime = [
            struct(
                deb = "libtinfo6_6.3-2ubuntu0.3_amd64.deb",
                libraries = {"libtinfo.so.6": "lib/x86_64-linux-gnu/libtinfo.so.6"},
                license = "X11",
                copyright = "usr/share/doc/libtinfo6/copyright",
                package = "libtinfo6",
                sha256 = "3b8dff43a31168f5020928f2ab9a97324c5b47ae708311781f6bd01306c96214",
            ),
            struct(
                deb = "zlib1g_1.2.11.dfsg-2ubuntu9.2_amd64.deb",
                libraries = {"libz.so.1": "lib/x86_64-linux-gnu/libz.so.1"},
                license = "Zlib",
                copyright = "usr/share/doc/zlib1g/copyright",
                package = "zlib1g",
                sha256 = "9dc17e51a1be2d9ed63b7b84ef0e4e29c5abe6f1bc62cb03e7181483cce8a2f2",
            ),
            struct(
                copyright = "usr/share/doc/gcc-12-base/copyright",
                deb = "gcc-12-base_12.3.0-1ubuntu1~22.04.3_amd64.deb",
                libraries = {},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                package = "gcc-12-base",
                sha256 = "7f9253b7e0976f0526fc21346c73ee006e185a7b2f2f865048ea78a2af55bc8d",
            ),
            struct(
                deb = "libgcc-s1_12.3.0-1ubuntu1~22.04.3_amd64.deb",
                libraries = {"libgcc_s.so.1": "lib/x86_64-linux-gnu/libgcc_s.so.1"},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                copyright = None,
                package = "libgcc-s1",
                sha256 = "d383e642d83263147f7b4b69fb92cf1036e5246c6be3accf10b77f986684589a",
            ),
            struct(
                deb = "libstdc++6_12.3.0-1ubuntu1~22.04.3_amd64.deb",
                libraries = {"libstdc++.so.6": "usr/lib/x86_64-linux-gnu/libstdc++.so.6"},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                copyright = None,
                package = "libstdc++6",
                sha256 = "29ef3d289b272704b75141906c8f683db0f2bdb0942061366691333bfb289bdc",
            ),
            struct(
                deb = "libssl3_3.0.2-0ubuntu1.30_amd64.deb",
                libraries = {"libcrypto.so.3": "usr/lib/x86_64-linux-gnu/libcrypto.so.3"},
                license = "Apache-2.0",
                copyright = "usr/share/doc/libssl3/copyright",
                package = "libssl3",
                sha256 = "d1b4636e32ac6d3b95d583ac05ac548b1aa248414bae1c51b14f54be0f8fd7bc",
            ),
        ],
    ),
    "linux_arm64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        otp_sha256 = "8e16954a07278e83de260489ed4af9bcca0e22ab7e99eb11de781ec6f01e95d9",
        otp_url = "https://builds.hex.pm/builds/otp/arm64/ubuntu-22.04/OTP-" + OTP_VERSION + ".tar.gz",
        strip_prefix = "OTP-" + OTP_VERSION,
        runtime = [
            struct(
                deb = "libtinfo6_6.3-2ubuntu0.3_arm64.deb",
                libraries = {"libtinfo.so.6": "lib/aarch64-linux-gnu/libtinfo.so.6"},
                license = "X11",
                copyright = "usr/share/doc/libtinfo6/copyright",
                package = "libtinfo6",
                sha256 = "971b5fd1cf8f7d68ba00f40d8eec6474fae6a6adbe19883aa42dd2b74c672e4a",
            ),
            struct(
                deb = "zlib1g_1.2.11.dfsg-2ubuntu9.2_arm64.deb",
                libraries = {"libz.so.1": "lib/aarch64-linux-gnu/libz.so.1"},
                license = "Zlib",
                copyright = "usr/share/doc/zlib1g/copyright",
                package = "zlib1g",
                sha256 = "cfd99d1ca41e710fdd3f68bfdc4d36dd24e14377656764f00b215b89cf6976e8",
            ),
            struct(
                copyright = "usr/share/doc/gcc-12-base/copyright",
                deb = "gcc-12-base_12.3.0-1ubuntu1~22.04.3_arm64.deb",
                libraries = {},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                package = "gcc-12-base",
                sha256 = "b2bc566681989941dd059d6391b01c41854857f26d83c53358287d620dfb0fd4",
            ),
            struct(
                deb = "libgcc-s1_12.3.0-1ubuntu1~22.04.3_arm64.deb",
                libraries = {"libgcc_s.so.1": "lib/aarch64-linux-gnu/libgcc_s.so.1"},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                copyright = None,
                package = "libgcc-s1",
                sha256 = "46626e1af6b159061e055f843654ee1f0f1e65fa280331ed47ee76ba1b1f6045",
            ),
            struct(
                deb = "libstdc++6_12.3.0-1ubuntu1~22.04.3_arm64.deb",
                libraries = {"libstdc++.so.6": "usr/lib/aarch64-linux-gnu/libstdc++.so.6"},
                license = "GPL-3.0-or-later WITH GCC-exception-3.1",
                copyright = None,
                package = "libstdc++6",
                sha256 = "59d244119e6129c7e9c0897491c0e210e925d15881b2dedcf8965ceb4fbf1aa3",
            ),
            struct(
                deb = "libssl3_3.0.2-0ubuntu1.30_arm64.deb",
                libraries = {"libcrypto.so.3": "usr/lib/aarch64-linux-gnu/libcrypto.so.3"},
                license = "Apache-2.0",
                copyright = "usr/share/doc/libssl3/copyright",
                package = "libssl3",
                sha256 = "1c4b8cc736eed8a0caafc385f854fa6852382221d84be4795f1d19a52b9fec53",
            ),
        ],
    ),
    "macos_arm64": struct(
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        otp_sha256 = "579af3bb043e0692503f7f24c6e65b620bdf1802a13567a7247d1d18f1ec5470",
        otp_url = "https://github.com/erlef/otp_builds/releases/download/OTP-" + OTP_VERSION + "/OTP-" + OTP_VERSION + "-macos-arm64.tar.gz",
        strip_prefix = "",
        runtime = [],
    ),
}

DROPPED_PATHS = ["Install", "misc"]

DROPPED_APPLICATIONS = ["wx"]

DROPPED_APPLICATION_DIRS = ["doc", "examples", "src"]
