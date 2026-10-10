"""Load tests pinning standalone-artifact metadata."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":actionlint.linux_arm64.bzl", _actionlint_linux_arm64 = "ARTIFACT")
load(":actionlint.linux_x86_64.bzl", _actionlint_linux_x86_64 = "ARTIFACT")
load(":actionlint.macos_arm64.bzl", _actionlint_macos_arm64 = "ARTIFACT")
load(":actionlint.windows_x86_64.bzl", _actionlint_windows_x86_64 = "ARTIFACT")
load(":biome.linux_arm64.bzl", _biome_linux_arm64 = "ARTIFACT")
load(":biome.linux_x86_64.bzl", _biome_linux_x86_64 = "ARTIFACT")
load(":biome.macos_arm64.bzl", _biome_macos_arm64 = "ARTIFACT")
load(":biome.windows_x86_64.bzl", _biome_windows_x86_64 = "ARTIFACT")
load(":buildifier.linux_arm64.bzl", _buildifier_linux_arm64 = "ARTIFACT")
load(":buildifier.linux_x86_64.bzl", _buildifier_linux_x86_64 = "ARTIFACT")
load(":buildifier.macos_arm64.bzl", _buildifier_macos_arm64 = "ARTIFACT")
load(":buildifier.windows_x86_64.bzl", _buildifier_windows_x86_64 = "ARTIFACT")
load(":extension.bzl", "TOOL_ARTIFACTS")
load(":gitleaks.linux_arm64.bzl", _gitleaks_linux_arm64 = "ARTIFACT")
load(":gitleaks.linux_x86_64.bzl", _gitleaks_linux_x86_64 = "ARTIFACT")
load(":gitleaks.macos_arm64.bzl", _gitleaks_macos_arm64 = "ARTIFACT")
load(":gitleaks.windows_x86_64.bzl", _gitleaks_windows_x86_64 = "ARTIFACT")
load(":repos.bzl", "DX_TOOL_HUB", "DX_TOOL_REPOS")
load(":ruff.linux_arm64.bzl", _ruff_linux_arm64 = "ARTIFACT")
load(":ruff.linux_x86_64.bzl", _ruff_linux_x86_64 = "ARTIFACT")
load(":ruff.macos_arm64.bzl", _ruff_macos_arm64 = "ARTIFACT")
load(":ruff.windows_x86_64.bzl", _ruff_windows_x86_64 = "ARTIFACT")
load(":shellcheck.linux_arm64.bzl", _shellcheck_linux_arm64 = "ARTIFACT")
load(":shellcheck.linux_x86_64.bzl", _shellcheck_linux_x86_64 = "ARTIFACT")
load(":shellcheck.macos_arm64.bzl", _shellcheck_macos_arm64 = "ARTIFACT")
load(":shellcheck.windows_x86_64.bzl", _shellcheck_windows_x86_64 = "ARTIFACT")
load(":staticcheck.linux_arm64.bzl", _staticcheck_linux_arm64 = "ARTIFACT")
load(":staticcheck.linux_x86_64.bzl", _staticcheck_linux_x86_64 = "ARTIFACT")
load(":staticcheck.macos_arm64.bzl", _staticcheck_macos_arm64 = "ARTIFACT")
load(":staticcheck.windows_x86_64.bzl", _staticcheck_windows_x86_64 = "ARTIFACT")
load(":taplo.linux_arm64.bzl", _taplo_linux_arm64 = "ARTIFACT")
load(":taplo.linux_x86_64.bzl", _taplo_linux_x86_64 = "ARTIFACT")
load(":taplo.macos_arm64.bzl", _taplo_macos_arm64 = "ARTIFACT")
load(":taplo.windows_x86_64.bzl", _taplo_windows_x86_64 = "ARTIFACT")
load(":ty.linux_arm64.bzl", _ty_linux_arm64 = "ARTIFACT")
load(":ty.linux_x86_64.bzl", _ty_linux_x86_64 = "ARTIFACT")
load(":ty.macos_arm64.bzl", _ty_macos_arm64 = "ARTIFACT")
load(":ty.windows_x86_64.bzl", _ty_windows_x86_64 = "ARTIFACT")
load(":vale.linux_arm64.bzl", _vale_linux_arm64 = "ARTIFACT")
load(":vale.linux_x86_64.bzl", _vale_linux_x86_64 = "ARTIFACT")
load(":vale.macos_arm64.bzl", _vale_macos_arm64 = "ARTIFACT")
load(":vale.windows_x86_64.bzl", _vale_windows_x86_64 = "ARTIFACT")

FROZEN_SCHEMA_KEYS = [
    "abi_floor",
    "archive",
    "checksum_source",
    "cpu",
    "delivery_class",
    "executable",
    "executable_sha256",
    "interpreter",
    "licenses",
    "linkage",
    "needed_shared_libraries",
    "os",
    "runtime_files",
    "schema_version",
    "sha256",
    "size",
    "tool",
    "upstream_version",
    "url",
]

def _artifact_checks(artifact, tool, platform, version, url, sha256, size, exe, exe_sha256):
    return [
        expect_equal(tool + " schema keys", sorted(artifact.keys()), FROZEN_SCHEMA_KEYS),
        expect_equal(tool + " schema version", artifact["schema_version"], 1),
        expect_equal(tool + " delivery class", artifact["delivery_class"], "standalone"),
        expect_equal(tool + " upstream version", artifact["upstream_version"], version),
        expect_equal(tool + " url", artifact["url"], url),
        expect_equal(tool + " sha256", artifact["sha256"], sha256),
        expect_equal(tool + " size", artifact["size"], size),
        expect_equal(tool + " executable", artifact["executable"], exe),
        expect_equal(tool + " executable sha256", artifact["executable_sha256"], exe_sha256),
        expect_equal(tool + " platform", artifact["os"] + "_" + artifact["cpu"], platform),
    ]

def metadata_tests(name):
    """Declare the standalone-artifact metadata pin test."""
    checks = []
    checks += _artifact_checks(
        _actionlint_linux_arm64,
        "actionlint",
        "linux_arm64",
        "1.7.12",
        "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_arm64.tar.gz",
        "325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6",
        2111482,
        "actionlint",
        "ac0323433c2853ec3fb978c611430c5b3dc5d43c58d1a1ec031b00ab572beb60",
    )
    checks += _artifact_checks(
        _actionlint_linux_x86_64,
        "actionlint",
        "linux_x86_64",
        "1.7.12",
        "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz",
        "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8",
        2353908,
        "actionlint",
        "c872d6db8c6bf83a8eaa704fc93999f027d55dffbc63b8a6abdccb47df5f4cd4",
    )
    checks += _artifact_checks(
        _actionlint_macos_arm64,
        "actionlint",
        "macos_arm64",
        "1.7.12",
        "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_darwin_arm64.tar.gz",
        "aba9ced2dee8d27fecca3dc7feb1a7f9a52caefa1eb46f3271ea66b6e0e6953f",
        2164202,
        "actionlint",
        "8db11704dc296f096216db4db65d86cd7f0ebfdf4c38453a1da276b137b88388",
    )
    checks += _artifact_checks(
        _actionlint_windows_x86_64,
        "actionlint",
        "windows_x86_64",
        "1.7.12",
        "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_windows_amd64.zip",
        "6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9",
        2479065,
        "actionlint.exe",
        "54ca21be3de4c7cfa26914aa8b61bd76bf573ef3caac5f80d110558cdf241718",
    )
    checks += _artifact_checks(
        _biome_linux_arm64,
        "biome",
        "linux_arm64",
        "2.5.12",
        "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-arm64",
        "4c1c9908e5cfd5d327e4ac3205baa13b1d3dfd24f184ed289a0c0ef1b2e0274f",
        57998768,
        "biome-linux-arm64",
        "4c1c9908e5cfd5d327e4ac3205baa13b1d3dfd24f184ed289a0c0ef1b2e0274f",
    )
    checks += _artifact_checks(
        _biome_linux_x86_64,
        "biome",
        "linux_x86_64",
        "2.5.12",
        "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-x64",
        "e2475688799c9e78dd25ba5cf676676ffe74caf182a35082b1d22039151fdf63",
        63652488,
        "biome-linux-x64",
        "e2475688799c9e78dd25ba5cf676676ffe74caf182a35082b1d22039151fdf63",
    )
    checks += _artifact_checks(
        _biome_macos_arm64,
        "biome",
        "macos_arm64",
        "2.5.12",
        "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-darwin-arm64",
        "0e8d513eb6c612236b47ccf0e218ac3917d863d41f853c65c38410774daa26ed",
        56017952,
        "biome-darwin-arm64",
        "0e8d513eb6c612236b47ccf0e218ac3917d863d41f853c65c38410774daa26ed",
    )
    checks += _artifact_checks(
        _biome_windows_x86_64,
        "biome",
        "windows_x86_64",
        "2.5.12",
        "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-win32-x64.exe",
        "59f88d04ab25f8634639c337ef70a79927504b6379a1ec3ff4e87f7c15dd5dce",
        74882048,
        "biome-win32-x64.exe",
        "59f88d04ab25f8634639c337ef70a79927504b6379a1ec3ff4e87f7c15dd5dce",
    )
    checks += _artifact_checks(
        _buildifier_linux_arm64,
        "buildifier",
        "linux_arm64",
        "8.5.1",
        "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-arm64",
        "947bf6700d708026b2057b09bea09abbc3cafc15d9ecea35bb3885c4b09ccd04",
        7483831,
        "buildifier-linux-arm64",
        "947bf6700d708026b2057b09bea09abbc3cafc15d9ecea35bb3885c4b09ccd04",
    )
    checks += _artifact_checks(
        _buildifier_linux_x86_64,
        "buildifier",
        "linux_x86_64",
        "8.5.1",
        "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-amd64",
        "887377fc64d23a850f4d18a077b5db05b19913f4b99b270d193f3c7334b5a9a7",
        7757842,
        "buildifier-linux-amd64",
        "887377fc64d23a850f4d18a077b5db05b19913f4b99b270d193f3c7334b5a9a7",
    )
    checks += _artifact_checks(
        _buildifier_macos_arm64,
        "buildifier",
        "macos_arm64",
        "8.5.1",
        "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-darwin-arm64",
        "62836a9667fa0db309b0d91e840f0a3f2813a9c8ea3e44b9cd58187c90bc88ba",
        7565746,
        "buildifier-darwin-arm64",
        "62836a9667fa0db309b0d91e840f0a3f2813a9c8ea3e44b9cd58187c90bc88ba",
    )
    checks += _artifact_checks(
        _buildifier_windows_x86_64,
        "buildifier",
        "windows_x86_64",
        "8.5.1",
        "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-windows-amd64.exe",
        "f4ecb9c73de2bc38b845d4ee27668f6248c4813a6647db4b4931a7556052e4e1",
        7861760,
        "buildifier-windows-amd64.exe",
        "f4ecb9c73de2bc38b845d4ee27668f6248c4813a6647db4b4931a7556052e4e1",
    )
    checks += _artifact_checks(
        _gitleaks_linux_arm64,
        "gitleaks",
        "linux_arm64",
        "8.30.1",
        "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_arm64.tar.gz",
        "e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080",
        7601421,
        "gitleaks",
        "00e91bbe655bd7c47753e8cfe61cb76ea1a5d7e7702fe161ee40102b46b3823b",
    )
    checks += _artifact_checks(
        _gitleaks_linux_x86_64,
        "gitleaks",
        "linux_x86_64",
        "8.30.1",
        "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz",
        "551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb",
        8230402,
        "gitleaks",
        "88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509",
    )
    checks += _artifact_checks(
        _gitleaks_macos_arm64,
        "gitleaks",
        "macos_arm64",
        "8.30.1",
        "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_arm64.tar.gz",
        "b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5",
        7897593,
        "gitleaks",
        "ba52fb1bfabbcde42f032afad3d6e0b19dff8ed105229a16e7caa338bbc0e84f",
    )
    checks += _artifact_checks(
        _gitleaks_windows_x86_64,
        "gitleaks",
        "windows_x86_64",
        "8.30.1",
        "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip",
        "d29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e",
        8438883,
        "gitleaks.exe",
        "17157e2ee8b76fc8b1d8bee607a250e34b8a8023c8bc81822d4b5ee4d78fcb7c",
    )
    checks += _artifact_checks(
        _ruff_linux_arm64,
        "ruff",
        "linux_arm64",
        "0.16.7",
        "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-unknown-linux-gnu.tar.gz",
        "1e06b11127c28387c8066da4ce6a617a84a359591be09dbf581fbb0c3e006239",
        9443710,
        "ruff-aarch64-unknown-linux-gnu/ruff",
        "0e3c9829ec8e95bcd8a36f6523a1fe6c30ae30ccae31d9408ecc2e64d975768b",
    )
    checks += _artifact_checks(
        _ruff_linux_x86_64,
        "ruff",
        "linux_x86_64",
        "0.16.7",
        "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-unknown-linux-gnu.tar.gz",
        "73894c7b7c9a53fd66ed715eb3a1ec65077f316328e377057a98bdb7fcba0326",
        9977676,
        "ruff-x86_64-unknown-linux-gnu/ruff",
        "fef5ab1d39e0c8368a3ef2511f33083f372943ac76fe4bb2831f312d0cd9a5bd",
    )
    checks += _artifact_checks(
        _ruff_macos_arm64,
        "ruff",
        "macos_arm64",
        "0.16.7",
        "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-apple-darwin.tar.gz",
        "80221a5e0b1ae29262a74496f2ad1380c1ab52b3edd8cee13ec76d8acff406ca",
        9344878,
        "ruff-aarch64-apple-darwin/ruff",
        "61da2ccac4b58c298f35544a59d2f52ab6bc4a8eacbe866edf7d20993206c569",
    )
    checks += _artifact_checks(
        _ruff_windows_x86_64,
        "ruff",
        "windows_x86_64",
        "0.16.7",
        "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-pc-windows-msvc.zip",
        "a099e761fb841fc33d44031aa37e16a3d154922fd3da284396e58ae687973e45",
        9764202,
        "ruff.exe",
        "bf56979a91062b3d1862874d65cfc17157f0c55d89a9a6615ebbfac615a4a270",
    )
    checks += _artifact_checks(
        _shellcheck_linux_arm64,
        "shellcheck",
        "linux_arm64",
        "0.11.0",
        "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.aarch64.tar.gz",
        "68a8133197a50beb8803f8d42f9908d1af1c5540d4bb05fdfca8c1fa47decefc",
        11668131,
        "shellcheck-v0.11.0/shellcheck",
        "127f13925eadd52c341bca0ebaf9ab0dbd78c6468f30a8f262a528bf8de47546",
    )
    checks += _artifact_checks(
        _shellcheck_linux_x86_64,
        "shellcheck",
        "linux_x86_64",
        "0.11.0",
        "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.gz",
        "b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6",
        3773312,
        "shellcheck-v0.11.0/shellcheck",
        "4da528ddb3a4d1b7b24a59d4e16eb2f5fd960f4bd9a3708a15baddbdf1d5a55b",
    )
    checks += _artifact_checks(
        _shellcheck_macos_arm64,
        "shellcheck",
        "macos_arm64",
        "0.11.0",
        "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.darwin.aarch64.tar.gz",
        "339b930feb1ea764467013cc1f72d09cd6b869ebf1013296ba9055ab2ffbd26f",
        11370575,
        "shellcheck-v0.11.0/shellcheck",
        "61c17246d69f012cd458ae82f244c46023dac75d1b69733ca1cc7d28fb270fd7",
    )
    checks += _artifact_checks(
        _shellcheck_windows_x86_64,
        "shellcheck",
        "windows_x86_64",
        "0.11.0",
        "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.zip",
        "8a4e35ab0b331c85d73567b12f2a444df187f483e5079ceffa6bda1faa2e740e",
        8068167,
        "shellcheck.exe",
        "c9e82ada36ef4b8d4caf1f97fa89289048c8f4a33c2c76ffffc88bfe09ff00c5",
    )
    checks += _artifact_checks(
        _staticcheck_linux_arm64,
        "staticcheck",
        "linux_arm64",
        "2026.2",
        "https://github.com/dominikh/go-tools/releases/download/2026.2/staticcheck_linux_arm64.tar.gz",
        "3315fa61b8e18512d43ce5bf4fe1bf9b55e85d2febafe6baa3847e243a484348",
        8538109,
        "staticcheck/staticcheck",
        "e64f135562f1984d68c5f22d8ea06e6c36e73fcf1ded79f6f50870d8090cc581",
    )
    checks += _artifact_checks(
        _staticcheck_linux_x86_64,
        "staticcheck",
        "linux_x86_64",
        "2026.2",
        "https://github.com/dominikh/go-tools/releases/download/2026.2/staticcheck_linux_amd64.tar.gz",
        "df1fd2b42de9fdef42231329522ea8f1f6846a5eb1a0200fad14c90bba464d02",
        9419376,
        "staticcheck/staticcheck",
        "ba8e1dd887ca0c6a60838fc880b023a137b1feac99bcc1566ba5533429c9b88e",
    )
    checks += _artifact_checks(
        _staticcheck_macos_arm64,
        "staticcheck",
        "macos_arm64",
        "2026.2",
        "https://github.com/dominikh/go-tools/releases/download/2026.2/staticcheck_darwin_arm64.tar.gz",
        "9e831155872d1982fe322e9ce146fc013541a8b71bc43371f94b20cc6fcf131e",
        8947431,
        "staticcheck/staticcheck",
        "960e43040c76bac7fba6900a25b212b1c04798e6e316e88619685b96b9cedcda",
    )
    checks += _artifact_checks(
        _staticcheck_windows_x86_64,
        "staticcheck",
        "windows_x86_64",
        "2026.2",
        "https://github.com/dominikh/go-tools/releases/download/2026.2/staticcheck_windows_amd64.tar.gz",
        "cc95236095badd515646d5a1b8b352deaf81614b4ac14f2652b7031ae5f67973",
        9503766,
        "staticcheck/staticcheck.exe",
        "111d26aae31530799c6ab562d860916bd1198ab68df30e85421439f12c98b405",
    )
    checks += _artifact_checks(
        _taplo_linux_arm64,
        "taplo",
        "linux_arm64",
        "0.10.0",
        "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-aarch64.gz",
        "033681d01eec8376c3fd38fa3703c79316f5e14bb013d859943b60a07bccdcc3",
        4631779,
        "taplo-aarch64",
        "82df9d765856d0d94d2147cc0912016e4a2bfb96cbe947347b7cc04c7f4431ba",
    )
    checks += _artifact_checks(
        _taplo_linux_x86_64,
        "taplo",
        "linux_x86_64",
        "0.10.0",
        "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-x86_64.gz",
        "8fe196b894ccf9072f98d4e1013a180306e17d244830b03986ee5e8eabeb6156",
        5116068,
        "taplo-x86_64",
        "dad2faf6377d2daa4f4fabf459fe7ccfb98a5448f0d4bca8270ca9acb0409bfe",
    )
    checks += _artifact_checks(
        _taplo_macos_arm64,
        "taplo",
        "macos_arm64",
        "0.10.0",
        "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-darwin-aarch64.gz",
        "713734314c3e71894b9e77513c5349835eefbd52908445a0d73b0c7dc469347d",
        4616415,
        "taplo",
        "13cd257c1cadb003b40daf82b3fb1451e012e2463b760bdd33df07a07970c604",
    )
    checks += _artifact_checks(
        _taplo_windows_x86_64,
        "taplo",
        "windows_x86_64",
        "0.10.0",
        "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-windows-x86_64.zip",
        "1615eed140039bd58e7089109883b1c434de5d6de8f64a993e6e8c80ca57bdf9",
        5182591,
        "taplo.exe",
        "12660d24becf05a78b39133112e91d1bd319b22b8e49660448c222a6858a6827",
    )
    checks += _artifact_checks(
        _ty_linux_arm64,
        "ty",
        "linux_arm64",
        "0.0.80",
        "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-unknown-linux-gnu.tar.gz",
        "c8a21f89ddea64bc6f79de733cdd005d02df0b98da55dc6376681f5d418af16e",
        12525064,
        "ty-aarch64-unknown-linux-gnu/ty",
        "48791915925a18e2009debfac679705fee95838d01085a018002d7841a68f283",
    )
    checks += _artifact_checks(
        _ty_linux_x86_64,
        "ty",
        "linux_x86_64",
        "0.0.80",
        "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-unknown-linux-gnu.tar.gz",
        "6c4142846197f39a3ac963a01512126eea2c01813c2fb4cd0ce7356fc1c9304b",
        13300460,
        "ty-x86_64-unknown-linux-gnu/ty",
        "147dde31480eb80efb7ea4646486505673fbd254c0239bbc95785467c3defe39",
    )
    checks += _artifact_checks(
        _ty_macos_arm64,
        "ty",
        "macos_arm64",
        "0.0.80",
        "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-apple-darwin.tar.gz",
        "745ad7e6e7aa410c50216488cacd25e2612f123dec535ec5408ae0aad4c3544d",
        12478461,
        "ty-aarch64-apple-darwin/ty",
        "9b09467ea3db2d4e507c0f420fa1fb7e9d7099969310fdddf7a293bdad6baa93",
    )
    checks += _artifact_checks(
        _ty_windows_x86_64,
        "ty",
        "windows_x86_64",
        "0.0.80",
        "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-pc-windows-msvc.zip",
        "f1adb0237d61295bd14b2f9e9095301174246d48f6c2772f15d141b7d91946e2",
        12690223,
        "ty.exe",
        "f2937abf5627a0f258d155c2425563ab1fdbb2a2df6104637977e92b2eaa0cef",
    )
    checks += _artifact_checks(
        _vale_linux_arm64,
        "vale",
        "linux_arm64",
        "3.20.0",
        "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_arm64.tar.gz",
        "d49e89479a40dbe6a6fe44a2963abef0ee39d89453f3c7100bdd1c5dd0708961",
        10969478,
        "vale",
        "1d60b298df01b7709acd46967e99035c90d0fccaa59cd6bc3e4f973a9d7f00f1",
    )
    checks += _artifact_checks(
        _vale_linux_x86_64,
        "vale",
        "linux_x86_64",
        "3.20.0",
        "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_64-bit.tar.gz",
        "f59e7030c5d4ace6cf915497d0d076a1699d61e876142765963237e6867c9712",
        11736086,
        "vale",
        "17bd39892f5c2e2f62c72d85be47facd83c21d39597154996134d15e34af4428",
    )
    checks += _artifact_checks(
        _vale_macos_arm64,
        "vale",
        "macos_arm64",
        "3.20.0",
        "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_macOS_arm64.tar.gz",
        "6b32df1a7d7b2ab01c07c1c4dd5fd84d775ac7a8f4101084cb5cc7df76bdc65e",
        11300418,
        "vale",
        "048f05392a0b26f52ce8f49a03d1b19d4dcc73dc6ab77e2cd9a9e9418a4f25dc",
    )
    checks += _artifact_checks(
        _vale_windows_x86_64,
        "vale",
        "windows_x86_64",
        "3.20.0",
        "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Windows_64-bit.zip",
        "189b3511813a428f08a2be0d7692e50dd6e90dd81b2f8e04dbfbfe42cafd9980",
        12000344,
        "vale.exe",
        "24ca0647d8b929d786cc371848fe1153e605361cd7dffa4badac9cdf33f99487",
    )

    derived_repos = sorted(["dx_%s_%s_%s" % (artifact["tool"], artifact["os"], artifact["cpu"]) for artifact in TOOL_ARTIFACTS])
    checks.append(expect_equal("dx tool repo count", len(DX_TOOL_REPOS), 40))
    checks.append(expect_equal("dx tool repos match metadata", DX_TOOL_REPOS, derived_repos))
    checks.append(expect_equal("dx tool repos sorted", DX_TOOL_REPOS, sorted(DX_TOOL_REPOS)))
    checks.append(expect_equal("dx tool hub", DX_TOOL_HUB, "dx_tools"))
    starlark_test(
        name = name,
        mode = "load",
        checks = checks,
    )
