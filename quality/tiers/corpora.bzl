"""Tier files each formatter owns, as paths under quality/tiers/."""

TIER_JAVASCRIPT = [
    "eslint/frameworks.mjs",
    "eslint/frameworks/astro.mjs",
    "eslint/frameworks/react.mjs",
    "eslint/frameworks/svelte.mjs",
    "eslint/frameworks/vue.mjs",
    "eslint/standard.mjs",
    "eslint/strict.mjs",
]

TIER_JSON = [
    "bicep/standard.json",
    "bicep/strict.json",
    "biome/standard.json",
    "biome/strict.json",
    "buildifier/standard.json",
    "buildifier/strict.json",
    "fsharplint/standard.json",
    "fsharplint/strict.json",
    "govet/standard.json",
    "govet/strict.json",
    "gplint/standard.json",
    "gplint/strict.json",
    "graphql_eslint/standard.json",
    "graphql_eslint/strict.json",
    "luals/standard.json",
    "luals/strict.json",
    "stylelint/standard.json",
    "stylelint/strict.json",
    "swift_format/standard.json",
    "swift_format/strict.json",
    "tsconfig/standard.json",
    "tsconfig/strict.json",
]

TIER_TOML = [
    "ffmt/standard.toml",
    "ffmt/strict.toml",
    "fortitude/standard.toml",
    "fortitude/strict.toml",
    "pydoclint/standard.toml",
    "pydoclint/strict.toml",
    "mbake/standard.toml",
    "mbake/strict.toml",
    "ruff/standard.toml",
    "ruff/strict.toml",
    "rustfmt/standard.toml",
    "rustfmt/strict.toml",
    "selene/standard.toml",
    "selene/strict.toml",
    "taplo/standard.toml",
    "taplo/strict.toml",
    "ty/standard.toml",
    "ty/strict.toml",
    "typos/standard.toml",
    "typos/strict.toml",
]

TIER_YAML = [
    "ansible_lint/standard.yaml",
    "ansible_lint/strict.yaml",
    "cfn_lint/standard.yaml",
    "cfn_lint/strict.yaml",
    "buf/standard.yaml",
    "buf/strict.yaml",
    "dart/standard.yaml",
    "dart/strict.yaml",
    "detekt/standard.yml",
    "detekt/strict.yml",
    "hadolint/standard.yaml",
    "hadolint/strict.yaml",
    "gersemi/standard.yaml",
    "gersemi/strict.yaml",
    "hlint/standard.yaml",
    "hlint/strict.yaml",
    "redocly/standard.yaml",
    "redocly/strict.yaml",
    "regal/standard.yaml",
    "regal/strict.yaml",
    "rubocop/standard.yml",
    "rubocop/strict.yml",
    "yamlfmt/standard.yaml",
    "yamlfmt/strict.yaml",
    "yamllint/standard.yaml",
    "yamllint/strict.yaml",
]

# typos rewrites selene's parenthese_conditions lint name, so
# this file stays out of corpus_toml. The taplo fixpoint case in quality/testdata still covers it.
TYPOS_UNSAFE = ["selene/strict.toml"]

def tier_labels(paths):
    """Maps tool/file paths to //quality/tiers/<tool>:<file> labels."""
    return ["//quality/tiers/%s:%s" % tuple(path.split("/", 1)) for path in paths]
