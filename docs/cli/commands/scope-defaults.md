# Scope Defaults

Most `dx` commands take Bazel scopes. This page lists what each command does
with a scope.

## No Scope Means `//...`

- `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx check`, `dx fix`, `dx security`, `dx license`, `dx migrate`,
  `dx list-tests`

## No Scope Means The Repository

- `dx generate`: runs Gazelle over the whole repo.
- `dx docs`: builds `//tools/docs/site:user_site`, or
  `//tools/docs/site:user_site_check` in `--check`.

## Repository-Wide Or One Exact Label

- `dx env`, `dx codegen`

Patterns, paths, and multiple labels are usage errors.

## Set Selectors

- `dx update`: set selectors. No selector updates every set.
- `dx bump`: exactly one `set:package` plus one version.

## Required Arguments

- `dx run`: needs a runnable scope.
- `dx deploy`: exactly one main-workspace label.
- `dx watch`: needs a watchable command plus its scopes.
- `dx new`: needs a language, plus an optional name.
- `dx init`: optional module name.
- `dx completion`: exactly one shell.

## No Scopes

- `dx clean`, `dx status`, `dx version`, `dx upgrade`, `dx rollback`

## Scope Shapes

- Labels and patterns: `//...`, `//pkg:target`, `@repo//...`.
- Files and dirs: workspace-relative paths resolved through `bazel query`
  to the owning targets. The ownership query covers the whole workspace, so
  owners in other packages are included.
- Paths with control characters in a file or directory name are refused.
- `--here`: the current directory tree (`//path/...`,
  `//...` at the root). Never combines with explicit scopes. Accepted by
  `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx generate`, `dx check`, `dx fix`, `dx security`,
  `dx license`, `dx list-tests`, and `dx docs`.

```sh
bazel run @rules_dx//:dx -- lint --here
bazel run @rules_dx//:dx -- build //cli/...
```
