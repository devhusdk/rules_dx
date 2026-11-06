# Ruff Config Owned By dx init

Verifies gazelle drops the root `ruff_config` and its `aspect_hints` when `.dx/config/BUILD.bazel`
already defines a ruff config for the root `ruff.toml`.
