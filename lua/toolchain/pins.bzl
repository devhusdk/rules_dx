"""Pinned LuaLS (lua-language-server) release archives."""

LUALS_VERSION = "3.19.2"

_RELEASE = "https://github.com/LuaLS/lua-language-server/releases/download/" + LUALS_VERSION + "/lua-language-server-" + LUALS_VERSION + "-"

# Keyed by "<os>_<arch>" as the repository rule derives it from the host.
LUALS_CELLS = {
    "darwin_aarch64": struct(
        archive = "darwin-arm64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "79149584320a23bd5fcd5bc31f47a1975acd8a4560a0e89036bb1016fa61687f",
        urls = [_RELEASE + "darwin-arm64.tar.gz"],
    ),
    "darwin_x86_64": struct(
        archive = "darwin-x64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "6f2174ece5ec63bcd9d5b22b2c902445c1cd9045fafe48c62602448ebf4c09cc",
        urls = [_RELEASE + "darwin-x64.tar.gz"],
    ),
    "linux_aarch64": struct(
        archive = "linux-arm64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "17b8e401a874440adeb649e5024c364f2bb9d15156c74a40e56d1ae9a1b2e4f2",
        urls = [_RELEASE + "linux-arm64.tar.gz"],
    ),
    "linux_x86_64": struct(
        archive = "linux-x64.tar.gz",
        binary = "bin/lua-language-server",
        sha256 = "93ad326b10f6c00911dd8c450fa098332dbd8f9f45d1052f5636a2d54479b947",
        urls = [_RELEASE + "linux-x64.tar.gz"],
    ),
    "windows_x86_64": struct(
        archive = "win32-x64.zip",
        binary = "bin/lua-language-server.exe",
        sha256 = "4a3dc851b3b31e7b75a23f645d1ba4a2137579a55f5fa4edbb17a3e2a6254044",
        urls = [_RELEASE + "win32-x64.zip"],
    ),
}
