"""The dx release that consumers download, with one sha256 per release asset (dx and dx_gazelle); empty means unpinned."""

DX_VERSION = "0.0.0"

URL_TEMPLATE = "https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}"

SHA256 = {
    "dx-linux-arm64": "",
    "dx-linux-x86_64": "",
    "dx-macos-arm64": "",
    "dx-windows-arm64": "",
    "dx-windows-x86_64": "",
}

GAZELLE_SHA256 = {
    "dx-gazelle-linux-arm64": "",
    "dx-gazelle-linux-x86_64": "",
    "dx-gazelle-macos-arm64": "",
    "dx-gazelle-windows-arm64": "",
    "dx-gazelle-windows-x86_64": "",
}
