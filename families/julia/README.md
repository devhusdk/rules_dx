# Julia

Julia test fixtures. They live in the `rules_dx_julia` module, next to
`rules_dx`. The rules, the tools, the lint, format and typecheck aspects and the
Gazelle language stay in `rules_dx`. The module selects the Julia toolchain and
fetches the packages the fixtures use, so a workspace that does not use Julia
does not fetch them.
