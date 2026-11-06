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
    "biome/standard.json",
    "biome/strict.json",
    "fsharplint/standard.json",
    "fsharplint/strict.json",
    "govet/standard.json",
    "govet/strict.json",
    "gplint/standard.json",
    "gplint/strict.json",
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
    "fortitude/standard.toml",
    "fortitude/strict.toml",
    "ruff/standard.toml",
    "ruff/strict.toml",
    "selene/standard.toml",
    "selene/strict.toml",
    "ty/standard.toml",
    "ty/strict.toml",
]

TIER_YAML = [
    "ansible_lint/standard.yaml",
    "ansible_lint/strict.yaml",
    "dart/standard.yaml",
    "dart/strict.yaml",
    "detekt/standard.yml",
    "detekt/strict.yml",
    "hlint/standard.yaml",
    "hlint/strict.yaml",
    "redocly/standard.yaml",
    "redocly/strict.yaml",
    "regal/standard.yaml",
    "regal/strict.yaml",
    "rubocop/standard.yml",
    "rubocop/strict.yml",
    "yamllint/standard.yaml",
    "yamllint/strict.yaml",
]

# typos rewrites selene's parenthese_conditions lint name and flags ruff's CPY rule prefix, so
# these files stay out of corpus_toml. The taplo fixpoint case in quality/testdata still covers them.
TYPOS_UNSAFE = ["ruff/strict.toml", "selene/strict.toml"]

def tier_labels(paths):
    """Maps tool/file paths to //quality/tiers/<tool>:<file> labels."""
    return ["//quality/tiers/%s:%s" % tuple(path.split("/", 1)) for path in paths]
