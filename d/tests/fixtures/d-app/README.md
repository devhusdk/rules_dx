# d-app

A D command line program with three module directories and one DUB package,
`semver`. It builds, runs and tests with the pinned LDC and needs no network
once the package is fetched.

```sh
tools/dx test //d/tests/fixtures/d-app/...
bazel run //d/tests/fixtures/d-app/source
```

Expected output:

```text
d-app 1.2.3
Hello, world!
semver orders
```

- `source/` holds `app.d` with `main`. `base/` and `cli/` hold the libraries.
- `source/base/banner.txt` is read at compile time with `import("banner.txt")`.
- `dub.selections.json` and `dub.digests.json` pin `semver`. The root
  `MODULE.bazel` maps its hub to `@d_app_deps`.
- `BUILD.bazel` files come from `dx generate`. The `d_test` targets and the
  `target_compatible_with` lines are written by hand.
- `diagnostics/` compiles broken sources with the pinned LDC and checks the
  messages.
