---
title: Native Windows bootstrap install tests
author: Martin
date: 2026-10-23
branch: issue-1741
cost: { minutes: 274, tokens: 513750 }
id: l-c6af81ed
scope: repo
status: active
touches:
  - cli/env/src/lib_tests.rs
  - cli/env/bootstrap_test.rs
---

## Decided
- `cli/env/src/lib_tests.rs` compares installed links through `installed_target` (canonicalize of read_link), the same form `verify_installed` uses. On Windows the installer stores a `\\?\C:\...` target but `fs::read_link` returns `C:\...`, so a raw read_link compare fails only on Windows.
- `workspace_path_with_spaces_installs` runs a native tool: on Windows it copies the test binary (`current_exe`) as the target and runs it with `--list`, since the shared fixture writes `#!/bin/sh` scripts that fail with os error 193. It installs with the host OS and `host_filename_for`, so Windows exercises `a.exe`.
## Watch out
- `//cli/env:bootstrap_test` asserts every generated host name is `<tool>` plus `EXE_SUFFIX`; a Windows run with POSIX-named metadata fails there, not later.
