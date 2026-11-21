"""The dx release that consumers download: one sha256 per tool and platform.

Written only by //tools/ci/fill_pins from the release manifests; empty on main.
The tools and platforms come from tools.bzl.
"""

# Loaded from outside rules_dx (consumer fixtures, generated BUILD files or docs).
visibility("public")

SCHEMA = 1

DX_VERSION = "0.0.1"

URL_TEMPLATE = "https://github.com/devhusdk/rules_dx/releases/download/v{version}/{asset}"

PINS = {}
