---
title: "rules_dx's test-only @maven install must stay a dev dependency or non-JVM consumers' dx lint fails"
author: Martin
date: 2026-10-23
branch: issue-1764
cost: { minutes: 1088, tokens: 1820525 }
id: l-8b7230de
scope: repo
status: active
touches:
  - MODULE.bazel.lock
  - examples/qt-app/README.md
  - examples/qt-app/app/BUILD.bazel
  - examples/qt-app/app/panel.ui
  - examples/qt-app/app/panel_test.cc
  - examples/qt-app/app/panel_test.h
  - examples/qt-app/app/tally_quick_test.cc
  - examples/qt-app/app/tst_tally.qml
  - qt/README.md
  - qt/rules/defs.bzl
---

## Watch out
- The lint aspect defaults `scalafix` to `//quality/tools/jvm:scalafix`, which depends on `@dx_scalafix` from the rules_jvm_external `maven` extension. Because of that, every consumer `dx lint` evaluates the extension, even a C++/QML workspace.
- A consumer without its own root `maven.install` fails with "Error in join: expected string ... got Label" when rules_dx's non-dev `maven.install(name = "maven", lock_file = ...)` collides with protobuf's `maven` lock file. That broke the quality case of `//qt/tests:qt_tasks_test`.

## Decided
- In `MODULE.bazel`, the junit/scalatest `maven` install uses `use_extension(..., dev_dependency = True)` because `@maven` is used only by rules_dx's own tests and examples. Only `dx_scalafix` goes through a separate non-dev proxy, because the lint aspect needs it in consumers.
