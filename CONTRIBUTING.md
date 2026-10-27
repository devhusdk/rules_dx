# Contributing

`rules_dx` is a Bazel developer platform. `dx` runs through Bazel.

```sh
tools/dx worktree add <branch>
tools/dx fix --apply --affected
tools/dx test --affected
tools/dx worktree release
```

Full run: `tools/dx test //...`

Read [AGENTS.md](AGENTS.md) before changing code.

- Treat warnings as errors.
- Run the formatter, linter, and focused tests for your change.
- Update tests and user docs when behavior changes.
- Do not edit generated files. Use the generator.
- Library crates return typed errors. Only binaries map exit codes.

## Issues and labels

Use the bug template. Paste `dx version` and `dx status`.

Each issue gets one priority label:

- `P0`: agent dev loop (worktrees, slots, fix, check, CI, launcher, JSON).
- `P1`: bugs in existing features.
- `P2`: improve and finish existing features.
- `P3`: new languages, platforms and features, last.

Other labels: `bug`, `enhancement`, `needs-host` (needs macOS, Windows, GPU, or signing secrets), `tracking`.
`dogfood` is an extra label, never the only one.

Filters: [user issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+-label%3Adogfood), [dogfood issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+label%3Adogfood).

## Dependency floors

Every consumer-facing (non-dev) `bazel_dep` in the root `MODULE.bazel` has two
versions, both recorded in `modules/versions.bzl`:

- `FLOOR_<DEP>`, in the `bazel_dep`: the oldest version consumers may resolve.
- `TESTED_<DEP>`, in a root-only `single_version_override` directly below it:
  the version the root builds and tests with. Consumers never see it.

A `# floor: <reason>` comment directly above each `bazel_dep` says why the floor
is what it is. Tools parse that marker, so keep it as written. Use
`dx bump bazel:<dep> <version>` to move the tested version, and
`dx bump bazel:<dep> <version> --floor --reason "<why>"` to move the floor.
`//modules:consumer_deps_test` checks that the two files agree.

For now every floor equals its tested version. Do not lower a floor until the
floor CI job exists. That job builds the consumer suites at the floors, and the
full procedure for finding a floor will land with it (#3747).
