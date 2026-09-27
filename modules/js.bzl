"""JavaScript/TypeScript foundation pins plus npm hubs."""

ASPECT_RULES_JS_VERSION = "3.4.1"
ASPECT_RULES_TS_VERSION = "3.10.0"
ASPECT_RULES_JEST_VERSION = "0.26.0"
TYPESCRIPT_VERSION = "5.9.3"
TYPESCRIPT_INTEGRITY = "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw=="
PNPM_VERSION = "10.34.5"
NODE_VERSION_PREFIX = "v22."

NPM_HUBS = ["npm", "npm_tools"]
NPM_LOCKS = [
    "//:pnpm-lock.yaml",
    "//quality/tools/javascript:pnpm-lock.yaml",
]

NPM_REPIN = "bazel run @pnpm//:pnpm -- update"
