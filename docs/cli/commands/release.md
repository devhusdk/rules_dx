# `dx release`

Works out the next version from conventional commits since the last tag and lists every manifest that carries a version. With `--apply` it also writes the version into every manifest, commits and tags. Without it, no files change.

```sh
dx release
dx release --check
dx release --check --commits origin/main..HEAD
dx release --apply
dx release --to 2.0.0 --apply
```

```text
dx release [--check [--commits <range>] | --apply [--push]] [--to <x.y.z>]
```

| Flag | What it does |
| --- | --- |
| `--check` | Report only. This is the default. |
| `--commits <range>` | With `--check`, lint the commit subjects in a git range such as `origin/main..HEAD`. Lists each one that is not a conventional commit and exits 1 when there is any. Merge commits are not read. Cannot be combined with `--apply` or `--to`. |
| `--apply` | Set the version in every manifest, run `bazel mod deps --lockfile_mode=update` when a `MODULE.bazel` changed, commit them as `release: <tag>` and create the tag. The tag is annotated and its message lists the commits since the last tag. Refuses a dirty working tree, a detached HEAD, an existing tag, or no version to release. Cannot be combined with `--check`. |
| `--push` | With `--apply`, push the branch and the tag to `origin`. Without it, the output says `next: git push --follow-tags`. |
| `--to <x.y.z>` | Use this version instead of the derived one, for teams that do not use conventional commits. |
| `--workspace`, `-q`, `-v`, `--color` | Global flags. |

The base is the newest tag that starts with the tag prefix (`v` by default). With no tag, the base is the version most manifests carry. Commits since the tag decide the bump: `feat:` is minor, `fix:` and `perf:` are patch, and `!` after the type or a `BREAKING CHANGE:` footer is major. Other commits do not bump.

The manifests are `Cargo.toml`, `package.json`, `pyproject.toml`, `pubspec.yaml`, `build.gradle(.kts)`, `*.csproj`, `mix.exs`, `MODULE.bazel`, `*.gemspec` and `Chart.yaml`. A manifest that does not carry the base version is listed as out of sync.

## release.toml

An optional `release.toml` at the workspace root.

| Key | Default | Meaning |
| --- | --- | --- |
| `tag_prefix` | `"v"` | The prefix of release tags. |
| `changelog` | `false` | When `true`, `--apply` prepends a `## [x.y.z] - date` section with the grouped notes to `CHANGELOG.md` (created with a Keep a Changelog header) and commits it; `--check` prints the section it would add. |
| `skip.subjects` | `[]` | Subject prefixes `--commits` lets through, for example `["Revert "]`. |

Scopes: none. Targets and `-- bazel-options` are usage errors. `--output` is not supported.
