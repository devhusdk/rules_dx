# Qt

Acquire a pinned Qt 6.11.3 SDK and its runtime closure without system
packages. Nothing is fetched until a target needs it.

```python
qt_sdk = use_extension("@rules_dx//qt/sdk:repos.bzl", "qt_sdk")

qt_sdk.configure(
    modules = ["qtdeclarative"],
)

use_repo(qt_sdk, "dx_qt")
```

`configure` takes:

- `modules`: `qtbase` (default), `qtdeclarative`, which adds `qtbase`, or
  `qttools`, which adds `qtdeclarative`. `qttools` carries only `lrelease`.
- `version`: must be `6.11.3`. Other versions fail.
- `mirror`: an https mirror of `download.qt.io/online/qtsdkrepository`.
- `runtime_mirror`: an https mirror of `vault.almalinux.org/9.6`, with its
  `BaseOS/` and `AppStream/` trees.

Call `configure` once.

## Targets

| Target | Module | What it is |
| --- | --- | --- |
| `@dx_qt//:core` | `qtbase` | QtCore headers and library |
| `@dx_qt//:network` | `qtbase` | QtNetwork headers and library |
| `@dx_qt//:gui` | `qtbase` | QtGui headers and library |
| `@dx_qt//:widgets` | `qtbase` | QtWidgets headers and library |
| `@dx_qt//:test` | `qtbase` | QtTest headers and library |
| `@dx_qt//:fonts` | `qtbase` | `fonts.conf` and the DejaVu Sans fonts |
| `@dx_qt//:platforms` | `qtbase` | `minimal` and `offscreen` platform plugins |
| `@dx_qt//:metatypes` | `qtbase` | Qt module metatypes JSON |
| `@dx_qt//:moc` | `qtbase` | `moc` launcher |
| `@dx_qt//:rcc` | `qtbase` | `rcc` launcher |
| `@dx_qt//:uic` | `qtbase` | `uic` launcher |
| `@dx_qt//:qml` | `qtdeclarative` | QtQml headers and library |
| `@dx_qt//:quick` | `qtdeclarative` | QtQuick headers and library |
| `@dx_qt//:quick_test` | `qtdeclarative` | QtQuickTest headers and library |
| `@dx_qt//:qml_imports` | `qtdeclarative` | QML import tree |
| `@dx_qt//:qtqml_qmldir` | `qtdeclarative` | `qml/QtQml/qmldir`, two levels below the import root |
| `@dx_qt//:qmlformat` | `qtdeclarative` | `qmlformat` launcher |
| `@dx_qt//:qmllint` | `qtdeclarative` | `qmllint` launcher |
| `@dx_qt//:qmltyperegistrar` | `qtdeclarative` | `qmltyperegistrar` launcher |
| `@dx_qt//:lrelease` | `qttools` | `lrelease` launcher |
| `@dx_qt//:runtime` | any | Qt, ICU, closure and graphics stack shared libraries |
| `@dx_qt//:licenses` | any | Qt, ICU and closure license texts |
| `@dx_qt//:sbom` | any | Qt SPDX documents |

A target from a module you did not enable fails analysis and names the
module.

## Formatting QML

`dx format` runs `@dx_qt//:qmlformat` when a `qml` family selects
`qmlformat`:

```sh
dx format --apply -- --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat
```

## Rules

Load the rules from `@rules_dx//qt/rules:defs.bzl`:

```python
load("@rules_dx//qt/rules:defs.bzl", "qt_cc_binary", "qt_cc_test", "qt_qml_module")

qt_qml_module(
    name = "tally",
    uri = "Tally",
    srcs = ["counter.cc"],
    moc_hdrs = ["counter.h"],
    qml_srcs = ["Main.qml"],
)

qt_cc_binary(
    name = "app",
    srcs = ["main.cc"],
    modules = ["qml"],
    resources = ["banner.txt"],
    resource_prefix = "/app",
    deps = [":tally"],
)

qt_cc_test(
    name = "counter_test",
    srcs = ["counter_test.cc"],
    moc_hdrs = ["counter_test.h"],
    modules = ["qml", "test"],
    deps = [":tally"],
)
```

Build and test for the Qt platform. Add to `.bazelrc`:

```text
build --platforms=@rules_dx//qt/platforms:linux_x86_64
build --use_target_platform_for_tests
```

The rules compile with `-fPIC` and link executables dynamically.

| Rule | Wraps | Default `modules` |
| --- | --- | --- |
| `qt_cc_library` | `cc_library` | `["core"]` |
| `qt_qml_module` | `cc_library` | `["qml"]` |
| `qt_cc_binary` | `cc_binary` | `["core"]` |
| `qt_cc_test` | `cc_test` | `["core", "test"]` |

Attributes on top of the `rules_cc` ones:

- `modules`: `@dx_qt` targets to link, such as `core`, `gui`, `widgets`, `qml`, `quick`, `quick_test` or `test`.
- `moc_hdrs`: headers with `Q_OBJECT` or `QML_ELEMENT`. `moc` runs on each.
- `forms`: `.ui` files for `uic`, which writes `ui_<name>.h`. Forms link `widgets`.
- `resources`: files compiled in with `rcc`, under `resource_prefix` (default `/`).
- `translations`: Qt Linguist `.ts` files. `lrelease` compiles each to `:/i18n/<name>.qm`. Needs `qttools`.
- `qt`: the Qt repo, `@dx_qt` by default. Pass `"@my_qt"` after `use_repo(qt_sdk, my_qt = "dx_qt")`.
- `qt_qml_module` only: `uri` (dotted, each part starts with a letter), `version` (`MAJOR.MINOR`, default `1.0`), `qml_srcs` and `qml_depends`. `qml_depends` lists the modules written as `depends` lines in the `qmldir`, such as `QtQml.Models` for a `QAbstractListModel` type.
- `qt_cc_binary` and `qt_cc_test` only: `qml_imports`. When true, `bazel run` and `bazel test` set `QML_IMPORT_PATH` to the pinned import tree. Defaults to true when `modules` holds `qml`, `quick` or `quick_test`.

`qt_qml_module` generates the `qmldir`, the `.qmltypes` and the type
registrations, and embeds the module at `qrc:/qt/qml/<uri path>/`. Load it
with `QQmlApplicationEngine::loadFromModule`. Its default outputs hold the
module directory, so `dx lint` passes it to `qmllint` as an import root.

`moc_hdrs`, `forms`, `resources`, `translations` and `qml_srcs` must live in
the target's own package. A file from another package fails analysis and
asks for its own target. Two translations with one file name fail analysis, also when two linked
targets each embed one.

## Gui, Widgets and Quick

A `qt_cc_binary` or `qt_cc_test` that links `gui` runs with the pinned
fonts and platform plugins: `bazel run` and `bazel test` set
`FONTCONFIG_PATH` and `QT_QPA_PLATFORM_PLUGIN_PATH`. Tests also set
`QT_QPA_PLATFORM=offscreen` and `QT_QUICK_BACKEND=software`, so they need no
display or GPU. For `bazel run`, pick the plugin yourself:

```sh
QT_QPA_PLATFORM=offscreen bazel run //app:viewer
```

Only `minimal` and `offscreen` are pinned. Any other plugin fails with
`Could not find the Qt platform plugin`. The target's `env` overrides these
variables.

Run Qt Quick Test suites through `quick_test_main`, and pass the `tst_*.qml`
files as `data`:

```python
qt_cc_test(
    name = "tally_quick_test",
    srcs = ["tally_quick_test.cc"],
    data = ["tst_tally.qml"],
    modules = ["quick_test"],
)
```

```cpp
#include <QtQuickTest/quicktest.h>

int main(int argc, char **argv) {
    return quick_test_main(argc, argv, "tally", "app");
}
```

The last argument is the test directory relative to the workspace root.

`dx lint` and `dx format` see only the hand-written C, C++ and QML files.
Generated `moc`, `uic`, `rcc` and registration sources are never checked.

## Packaging

`qt_bundle` stages what a `qt_cc_binary` runs with: its shared libraries
under `lib/`, the fonts under `fonts/` (an app must point `FONTCONFIG_FILE` at `fonts/fonts.conf` itself to use them), the platform plugins under `plugins/platforms/` when it links
`gui`, the QML import modules under `qml/` and the Qt, ICU and closure
license texts under `licenses/`. `lib/` holds the libraries the
binary links, the libraries those need, and the ones the shipped QML
plugins need. Pass it to `consumer_archive` as a
resource:

```python
load("@rules_dx//deploy/rules:archive.bzl", "consumer_archive")
load("@rules_dx//qt/rules:defs.bzl", "qt_bundle")

qt_bundle(
    name = "runtime",
    binary = ":app",
)

consumer_archive(
    name = "app_release",
    app = ":app",
    arch = "x86_64",
    min_runtime = "glibc-2.34",
    notice = "NOTICE",
    os = "linux",
    resources = [":runtime"],
)
```

- `binary`: the `qt_cc_binary`.
- `qml_modules`: import modules to ship. Defaults to `["QML", "QtQml"]` when
  the binary uses `qml`. A module missing from the import tree fails analysis.
- `licenses`: extra license texts to ship, such as `fmt/LICENSE` for a bundled library. Each path needs a `licenses/<name>/` directory and lands under `licenses/<name>/`.
- `qt`: the Qt repo, `@dx_qt` by default.

`qt_cc_binary` links with RUNPATH `$ORIGIN/resources/lib`, so the extracted
archive finds its libraries from any directory with no environment. Qt finds
`resources/qml` from the location of `libQt6Core`. A `qt.conf` beside the
binary overrides these paths.

## Linking Qt

Build Qt code for `@rules_dx//qt/platforms:linux_x86_64`, or a platform
with the same `@llvm` constraints (`libstdcxx.11.5.0`, `gnu.2.34`). Without
the rules, compile with `-fPIC` and link dynamically:

```python
cc_binary(
    name = "app",
    srcs = ["main.cc"],
    copts = ["-fPIC"],
    linkstatic = False,
    deps = ["@dx_qt//:qml"],
)
```

The launchers set `LD_LIBRARY_PATH` to the pinned closure and drop
`QT_PLUGIN_PATH`, `QML_IMPORT_PATH`, `QML2_IMPORT_PATH`, `QT_CONF_PATH`
and `QT_QPA_PLATFORM_PLUGIN_PATH`. Only glibc 2.34 or newer comes from
the host.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified: Core, Network, QML, QtTest, Gui, Widgets, Quick and Quick Test on the `offscreen` plugin with software rendering, `moc`, `rcc`, `uic`, `qmltyperegistrar`, `lrelease`, `qmlformat`, `qmllint`, `qt_bundle` archives. Not qualified: a real display, the `xcb` and `wayland` plugins, GPU rendering |
| `linux_arm64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

Other platforms fail analysis and list the qualified cell. Fetching needs a
Linux x86_64 host.
