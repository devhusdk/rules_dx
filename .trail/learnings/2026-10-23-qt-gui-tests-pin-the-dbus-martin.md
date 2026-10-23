---
title: "Qt Gui tests: pin the dbus closure and check host leaks from a transitioned binary"
author: Martin
date: 2026-10-23
branch: issue-1763
cost: { minutes: 730, tokens: 1236865 }
id: l-2c7061fb
scope: repo
status: active
touches:
  - BUILD.bazel
  - MODULE.bazel
  - acceptance/testdata/test_targets.txt
  - acceptance/v1_inventory.bzl
  - config/BUILD.bazel
  - docs/cli/commands/quality.md
  - erlang/README.md
  - erlang/rules/BUILD.bazel
  - erlang/rules/defs.bzl
  - erlang/rules/wrapper_tests.bzl
---

## Decided
- `libQt6Gui` links `libQt6DBus`, so the gui closure in `qt/sdk/pins.bzl` pins dbus-libs, systemd-libs, libcap, libgcrypt, libgpg-error and lz4-libs. Without them the loader takes the host copies and the `/proc/self/maps` check in `qt/tests/gui_test.cc` fails.
- The strict host-leak check lives in `qt/tests/qt_test.rs` and runs `:gui_check_linux_x86_64`, a `platform_transition_binary` of a `qt_cc_binary`. On the default repo platform (libc++), `libstdc++.so.6` comes from the host, so the in-process check in `gui_test.cc` allows it.
- Every hub `cc_library` in `qt/sdk/repos.bzl` defines its `QT_<X>_LIB` macro, as CMake does. `<QTest>` only declares `qWaitForWindowExposed` and the widget helpers when `QT_GUI_LIB` and `QT_WIDGETS_LIB` are set.
- Qt Quick Test suites call `quick_test_main(argc, argv, name, "<package dir>")` and ship `tst_*.qml` as `data`, because `qt_cc_test` does not forward `args` to the upstream `cc_test`.
## Tried and ruled out
- `platform_transition_test` on a `qt_cc_test`: it fails with "No matching toolchains for default_test_toolchain_type" because the Qt platform has no test toolchain.
- Expecting only "DejaVu Sans" from `QFontDatabase::families()`: Qt also adds Monospace, Sans Serif and Serif, which all resolve to DejaVu Sans.
- Requiring `libqtquick2plugin.so` to be loaded: `import QtQuick` resolves without the optional plugin.
## Watch out
- `bsdtar`-style `7zzs x -so | 7zzs x -si -tcpio` does not extract the AlmaLinux rpms. `_acquire_rpm` extracts the rpm, then the inner cpio, into directories.
