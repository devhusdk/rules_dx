---
title: MSIX packaging builds on Windows from the NuGet Build Tools
author: Martin
date: 2026-10-23
branch: issue-1805
cost: { minutes: 978, tokens: 1721343 }
id: l-c809dadd
scope: repo
status: active
touches:
  - msix/defs.bzl
  - msix/repos.bzl
  - msix/tests/BUILD.bazel
  - examples/msix-app/app/main.c
  - tools/docs/site/site_tests.bzl
---

## Decided
- `makeappx` and `signtool` come from the `Microsoft.Windows.SDK.BuildTools` NuGet package (`msix/pins.bzl`), fetched only after the root module calls `msix_tools.license(accept = ["windows-sdk-buildtools"])`. Without it `@dx_msix_tools` holds `msix_unavailable` stubs and `dx_msix` fails analysis with the license message. rules_dx accepts it as a dev dependency in its own `MODULE.bazel`.
- `//msix/tests:pack_test` runs `msix/pack.ps1` on Linux against cross-compiled PE files and the `fake_makeappx.sh` stand-in. The real `inspect_test`, `sign_test` and the manual `install_test` are Windows-only.
## Watch out
- `examples/msix-app/app/main.c` keeps `<appmodel.h>` in its own include block. clang-format (LLVM style) sorts it before `<windows.h>` otherwise, and it does not compile without `<windows.h>` first.
- Windows `C:\bz\cache\repos\v1\contents` can hold empty entry directories after a Bazel server is killed. Bazel then fails with "No MODULE.bazel, REPO.bazel, or WORKSPACE file found in C:/bz/cache/repos/v1/contents/...". Delete the empty directories and their `.recorded_inputs` files.
- `//tools/docs/site:site_unit` counts user book routes; the MSIX App page made it 48.
