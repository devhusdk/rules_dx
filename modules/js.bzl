"""JavaScript/TypeScript foundation pins plus npm hubs."""

load(":versions.bzl", _NODE_VERSION_PREFIX = "NODE_VERSION_PREFIX", _TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION")

TYPESCRIPT_VERSION = _TYPESCRIPT_VERSION
NODE_VERSION_PREFIX = _NODE_VERSION_PREFIX

NPM_HUBS = ["dx_npm", "dx_npm_tools"]
NPM_DEV_HUBS = ["npm_node_app"]
NPM_LOCKS = [
    "//:pnpm-lock.yaml",
    "//quality/tools/javascript:pnpm-lock.yaml",
    "//examples/node-app:pnpm-lock.yaml",
]
