# Scope Defaults

Most `dx` commands take Bazel scopes. This page lists what each command does
with a scope.

## No Scope Means `//...`

- `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx check`, `dx fix`, `dx security`, `dx license`, `dx migrate`

## No Scope Means The Repository

- `dx generate`: runs Gazelle over the whole repo.
- `dx docs`: builds `//docs/site:user_site`, or
  `//docs/site:user_site_aggregate` in `--check`.

## Repository-Wide Or One Exact Label

- `dx env`, `dx codegen`, `dx setup`

Patterns, paths, and multiple labels are usage errors.

## Set Selectors

- `dx update`: set selectors. No selector updates every set.
- `dx bump`: exactly one `set:package` plus one version.

## Required Arguments

- `dx run`: needs a runnable scope.
- `dx deploy`: exactly one main-workspace label.
- `dx why`: exactly one file plus one label.
- `dx owners`, `dx deps`: need at least one scope.
- `dx watch`: needs a watchable command plus its scopes.
- `dx hooks`: needs a verb. `run` needs a trigger.
- `dx new`: needs a language, plus an optional name.
- `dx init`: optional module name.
- `dx completion`: exactly one shell, or none with `--check`.

## No Scopes

- `dx clean`, `dx status`, `dx version`, `dx upgrade`

## Raw Bazel Args

- `dx bazel`: forwards args to Bazel. No scope resolution.

## Scope Shapes

- Labels and patterns: `//...`, `//pkg:target`, `@repo//...`.
- Files and dirs: workspace-relative paths resolved through `bazel query`
  to the owning targets.
- Paths with control characters in a file or directory name are refused.
- `--here` (`--cwd` alias): the current directory tree (`//path/...`,
  `//...` at the root). Never combines with explicit scopes. Accepted by
  `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx generate`, `dx check`, `dx fix`, `dx security`,
  `dx license`, and `dx docs`.

```sh
bazel run //cli/cli:dx -- lint --here
bazel run //cli/cli:dx -- build //cli/...
```
