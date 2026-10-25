# `dx docs`

```text
dx docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...] [-- bazel-options...]
```

Builds the documentation site with Bazel and the pinned upstream mdBook:
every `dx` command page, the GitHub CI guide, and the examples.
With no scope it builds the repository site (`//tools/docs/site:user_site`,
`//tools/docs/site:user_site_check` in `--check`). Args after `--` go to Bazel unchanged.
The build also passes
`--@rules_dx//config:validate=false`, `--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.

Scopes: Bazel labels or patterns (`//...`, `//pkg:target`, `@repo//...`), or
workspace-relative files or dirs resolved through Bazel query. No scope builds
`//tools/docs/site:user_site`, or `//tools/docs/site:user_site_check` with
`--check`. `--here` builds the current directory tree (`//path/...`, or `//...`
at the root) and cannot be combined with explicit scopes (exit `2`).

- `--check`: validate the book without rendering it. Builds the generated
  summary and landing page only.
- `--serve`: build, then preview the built site tree locally with
  `python3 -m http.server`. Needs `python3` on `PATH` and exactly one built
  site tree with an `index.html` entry page.
- `--port <n>`: preview port, `1`-`65535`, default `8000`. `0` is a usage error.
- `--host <addr>`: preview bind address, non-empty, default `127.0.0.1`.
- `--open`: open the preview URL in a browser through `python3`. If that
  fails, `dx` prints a warning and the preview keeps running.

`--port`, `--host`, and `--open` need `--serve`.

Output: `--output text|json`. `--output diff` is a usage error (exit `2`).
Text prints `Running docs build for <scope> (mdBook render)`, or
`Running docs check for <scope> (no render)` with `--check`. With `--serve`
it then prints `Serving docs at <url> (<dir>)`, and `Opening <url>` with
`--open`. JSON starts with `command_started` (mode `check` or `default`),
then `operation` events (`summary`, plus `render` without `--check`), then
one `error` event on failure (`invalid_arguments`, `bazel_failed`,
`serve_failed`, or `bazel_signalled`), and ends with `command_finished`.
Under `--serve`, `command_finished` comes when the preview server exits.

Reports: none. `--report` is a usage error.

Exit codes: `0` success, `2` usage or scope errors, including `--here` with
an explicit scope, `--output diff`, and `--report`. `1` a launch, signal, or
preview failure, including `serve_failed` (no single site tree, a missing
`index.html`, or a preview that fails to launch) and a signalled preview.
Bazel failures keep Bazel's code. Under `--serve` the preview server's code
is the exit code.

```sh
bazel run @rules_dx//:dx -- docs --check
bazel run @rules_dx//:dx -- docs
bazel run @rules_dx//:dx -- docs --serve --port 8080
bazel run @rules_dx//:dx -- docs --here
bazel run @rules_dx//:dx -- docs --serve --host 0.0.0.0 --port 8080 --open //tools/docs/site:user_site
bazel run @rules_dx//:dx -- docs --check --output=json
bazel run @rules_dx//:dx -- docs --check -- --config=ci
```

The build never changes sources. Serve runs a local preview only. The site
renders Markdown to HTML with syntax-highlighted code, a sidebar, a search
index, and one route per page.
