# `dx docs`

```text
dx docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...]
```

Builds the docs site with Bazel: all user guides plus the API reference.
With no scope it builds the repository site (`//docs/site:user_site`,
`//docs/site:user_site_aggregate` in `--check`). `--here` limits to the
current directory tree.

- `--check`: validate without rendering. Builds extract plus aggregate only.
- `--serve`: preview the last build locally after building.
- `--port <n>`, `--host <addr>`, `--open`: need `--serve`. `--open` opens
  the preview in a browser.
- `--output text|json`: result shape. `diff` has no patch.

```sh
bazel run //cli/cli:dx -- docs --check
bazel run //cli/cli:dx -- docs
bazel run //cli/cli:dx -- docs --serve --port 8080
```

The build never changes sources. Serve runs a local preview only. The demo
targets (`//docs/site:demo_*`) stay fixture-only for tests and are never
published.
