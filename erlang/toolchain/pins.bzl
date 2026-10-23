"""Pinned Erlang/OTP runtime, native closure and rules_erlang identities."""

OTP_VERSION = "29.1.1"

ERTS_VERSION = "17.1"

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
        otp_sha256 = "92d4f05939c51fc2d9e1b2b7d35b233e800d51a0e540767335f1d8680ea9a924",
        otp_url = "https://builds.hex.pm/builds/otp/amd64/ubuntu-22.04/OTP-" + OTP_VERSION + ".tar.gz",
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
}

DROPPED_PATHS = ["Install", "misc"]

DROPPED_APPLICATIONS = ["wx"]

DROPPED_APPLICATION_DIRS = ["doc", "examples", "src"]
