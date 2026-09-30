# `dx docs`

```text
dx docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...] [-- bazel-options...]
```

Builds the docs site with Bazel: all user guides plus the API reference.
With no scope it builds the repository site (`//docs/site:user_site`,
`//docs/site:user_site_aggregate` in `--check`). `--here` limits to the
current directory tree. Args after `--` go to Bazel unchanged.

- `--check`: validate without rendering. Builds extract plus aggregate only.
- `--serve`: preview the last build locally after building.
- `--port <n>`, `--host <addr>`, `--open`: need `--serve`. `--open` opens
  the preview in a browser.
- `--output text|json`: result shape. `diff` has no patch.

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal, or
preview failure. Bazel failures keep Bazel's code. Under `--serve` the
preview server's code is the exit code.

```sh
bazel run //cli/cli:dx -- docs --check
bazel run //cli/cli:dx -- docs
bazel run //cli/cli:dx -- docs --serve --port 8080
```

The build never changes sources. Serve runs a local preview only. The demo
targets (`//docs/site:demo_*`) stay fixture-only for tests and are never
published.
