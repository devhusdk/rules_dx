---
title: "ex_unit_test srcs order is load order, so the wrapper puts test_helper.exs first"
author: Martin
date: 2026-10-23
branch: issue-1790
cost: { minutes: 91, tokens: 512088 }
id: l-daa90607
scope: repo
status: active
touches:
  - elixir/rules/defs.bzl
  - elixir/rules/wrapper_tests.bzl
  - elixir/README.md
  - cli/adopt/src/new.rs
  - elixir/tests/starter/starter_test.rs
  - quality/adapter/src/parsers/mix_format.rs
  - tools/docs/site/site_tests.bzl
  - tools/docs/site/user/book.bzl
---

## Decided
- `ex_unit_test` in `elixir/rules/defs.bzl` passes `srcs` through `helper_first`. Upstream rules_elixir (`private/ex_unit_test.bzl`) turns each src into `-r <file>` in list order, and buildifier sorts `srcs`, so a sorted list would load `test_helper.exs` last. BUILD files, the `dx new elixir` starter (`cli/adopt/src/new.rs`) and the negative fixtures keep `srcs` sorted.
- `mix format` writes the unformatted path list ANSI-styled even with ANSI off; `quality/adapter/src/parsers/mix_format.rs` strips escapes and treats a styled line as a path.

## Watch out
- `dx new` walks up to the nearest `MODULE.bazel`. Inside a Bazel test that is `execroot/_main`, so `elixir/tests/starter/starter_test.rs` passes `--workspace <scratch>`.
- `//tools/docs/site:site_unit` counts user book routes (`tools/docs/site/site_tests.bzl`). Each new `mdbook_page` in `tools/docs/site/user/book.bzl` needs the count bumped.
