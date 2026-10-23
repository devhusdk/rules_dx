---
title: "buf format shells out to diff and needs PATH, so format checks run per file"
author: Martin
date: 2026-10-23
branch: issue-1774
cost: { minutes: 24, tokens: 136499 }
id: l-edf88897
scope: repo
status: active
touches:
  - ".github/workflows/apple.yml"
  - acceptance/workflow_capabilities.bzl
  - acceptance/workflow_capability_tests.bzl
  - apple/README.md
  - apple/ios/BUILD.bazel
  - apple/ios/app.bzl
  - apple/ios/app_tests.bzl
  - apple/ios/cell_tests.bzl
  - apple/ios/library_check.sh
  - apple/ios/xcframework_check.sh
---

## Decided
buf 1.72.0 execs `diff` whenever a file would change, even for plain `buf format` to stdout. The hermetic runner env has no PATH, so the aspect passes PATH=/usr/bin:/bin to buf (quality/real_aspects.bzl). The format check runs `buf format --path f root` per file and compares stdout with the input (quality/runner/src/real/check.rs, quality/adapter/src/parsers/buf.rs).

## Tried and ruled out
`buf format --diff --exit-code` batched over many files: needs diff too and gives no per-file result without parsing the diff.

## Watch out
Proto sources only reach the aspect because upstream_source_class maps `proto` to `protobuf` (quality/sources.bzl). Lint needs no HOME when the cwd is the scratch root.
