"""The dx release that consumers download, with one sha256 per release asset; empty means unpinned."""

DX_VERSION = "0.0.0"

URL_TEMPLATE = "https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}"

SHA256 = {
    "dx-linux-arm64": "",
    "dx-linux-x86_64": "",
    "dx-macos-arm64": "",
    "dx-windows-arm64": "",
    "dx-windows-x86_64": "",
}
