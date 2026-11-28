"""The dx release that consumers download: one sha256 per tool and platform."""

visibility("public")

SCHEMA = 1

DX_VERSION = "0.0.1"

URL_TEMPLATE = "https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}"

PINS = {}
