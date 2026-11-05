# `dx release`

Works out the next version from conventional commits since the last tag and lists every manifest that carries a version. It changes no files.

```sh
dx release
dx release --check
dx release --to 2.0.0
```

```text
dx release [--check] [--to <x.y.z>]
```

| Flag | What it does |
| --- | --- |
| `--check` | Report only. This is the default. |
| `--to <x.y.z>` | Use this version instead of the derived one, for teams that do not use conventional commits. |
| `--workspace`, `-q`, `-v`, `--color` | Global flags. |

The base is the newest tag that starts with the tag prefix (`v` by default). With no tag, the base is the version most manifests carry. Commits since the tag decide the bump: `feat:` is minor, `fix:` and `perf:` are patch, and `!` after the type or a `BREAKING CHANGE:` footer is major. Other commits do not bump.

The manifests are `Cargo.toml`, `package.json`, `pyproject.toml`, `pubspec.yaml`, `build.gradle(.kts)`, `*.csproj`, `mix.exs`, `MODULE.bazel`, `*.gemspec` and `Chart.yaml`. A manifest that does not carry the base version is listed as out of sync.

## release.toml

An optional `release.toml` at the workspace root.

| Key | Default | Meaning |
| --- | --- | --- |
| `tag_prefix` | `"v"` | The prefix of release tags. |

Scopes: none. Targets and `-- bazel-options` are usage errors. `--output` is not supported.
