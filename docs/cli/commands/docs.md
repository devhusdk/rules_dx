# `dx docs`

```text
dx docs [--check] [--apply] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...] [-- bazel-options...]
```

Builds the documentation site with Bazel and the pinned upstream mdBook:
every `dx` command page, the GitHub CI guide, and the examples.
With no scope it builds the repository site (`//docs/site:user_site`,
`//docs/site:user_site_check` in `--check`). `--here` limits to the
current directory tree. Args after `--` go to Bazel unchanged.

- `--check`: validate the book without rendering it. Builds the generated
  summary and landing page only.
- `--serve`: preview the last build locally after building. Serves the
  rendered site tree, so every page has its own URL. Needs `--apply`:
  without it `dx` builds and names the serve command instead of launching
  the preview or the browser.
- `--port <n>`, `--host <addr>`, `--open`: need `--serve`. `--open` opens
  the preview in a browser.
- `--output text|json`: result shape. `diff` has no patch.

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal, or
preview failure. Bazel failures keep Bazel's code. Under `--serve` the
preview server's code is the exit code.

```sh
bazel run @rules_dx//:dx -- docs --check
bazel run @rules_dx//:dx -- docs
bazel run @rules_dx//:dx -- docs --serve --apply --port 8080
```

The build never changes sources. Serve runs a local preview only. The site
renders Markdown to HTML with syntax-highlighted code, a sidebar, a search
index, and one route per page. Internal links are relative, so the same
build serves at a domain root and from a project subdirectory. The demo
targets (`//docs/site:demo_*`) stay fixture-only for tests and are never
published.
