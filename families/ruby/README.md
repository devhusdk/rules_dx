# Ruby

Ruby test fixtures. They live in the `rules_dx_ruby` module, next to
`rules_dx`. The rules, the toolchain extensions, the lint, format and typecheck
aspects and the Gazelle language stay in `rules_dx`. The module registers the
Ruby toolchains and fetches the gems the fixtures use, so a workspace that does
not use Ruby does not fetch them.
