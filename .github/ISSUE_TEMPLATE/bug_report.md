---
name: Bug Report
about: Report a documentation error or a reproducible implementation defect.
labels: bug
---

# Bug Report

For documentation issues, link the affected text.

## dx --version

Output of `dx --version`.

## Bazel version

Output of `bazel --version` only. Not the contents of `.bazelversion`.

## OS and CPU

OS name and version, and CPU (`x86_64` or `arm64`):

- Windows: the build from `winver` or `ver`.
- macOS: the output of `sw_vers`.
- Linux: the output of `uname -srm`.

## dx status

Output of `dx status`:

```text
```

## Commands run

Exact commands.

## Expected Behavior

## Actual Behavior

## Minimal reproduction

A project from `dx init --apply <dir>` with a few sources is ideal.

## Logs

Remove credentials and unrelated private data.
