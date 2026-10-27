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
- `runtime_mirror`: an https mirror of `vault.almalinux.org`, with its
  `9.6/` and `10.1/` `BaseOS/` and `AppStream/` trees.

Call `configure` once.

## Your own cell

A cell is one platform's Qt archives and runtime closure. Declare your own
with `qt_sdk.cell` in the root module:

```python
qt_sdk.cell(
    name = "custom_x86_64",
    constraints = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
        "//platforms:custom",
    ],
    spec = "//third_party/qt:custom_x86_64.json",
)
```

`cell` takes:

- `name`: lowercase letters, digits and `_`. The cell is fetched into
  `@dx_qt_<name>`, and `@dx_qt//:core` and the other targets route to it.
  A pinned cell of the same name (`linux_x86_64`, `linux_arm64`,
  `macos_arm64`, `windows_x86_64`) is replaced.
- `constraints`: the platform constraints the cell serves. They must differ
  from every other cell's.
- `layout`: the file layout of the archives. Only `linux` (the default).
- `spec`: a `.json` file with `archives`, `directory`, `release` and
  `runtime`, and optionally `needs`: the `library`, `plugin` and
  `runtime_plugin` tables of what each shared object needs at run time.

Start from a pinned spec:

```sh
bazel build @rules_dx//qt/sdk:linux_x86_64_cell
cp bazel-bin/external/rules_dx+/qt/sdk/linux_x86_64_cell.json third_party/qt/custom_x86_64.json
```

The spec is checked like the pinned cells. Every archive and runtime package
needs a sha256. Archives come from `mirror` (or `download.qt.io`) under
`directory`. Runtime packages come from `runtime_mirror` (or
`vault.almalinux.org`). The Qt release must be 6.11.3. Unknown keys fail.

Only the root module may declare cells. `qt_bundle` packages a cell only
when its spec carries `needs`, and those tables must match its archives;
without them `@dx_qt//:needs` fails analysis and says why.

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
| `@dx_qt//:display_wayland` | `qtbase` | `wayland` platform plugin, `xdg-shell` and `bradient` plugins |
| `@dx_qt//:display_xcb` | `qtbase` | `xcb` platform plugin and its `glx` and `egl` GL integration plugins |
| `@dx_qt//:plugins_imageformats` | `qtbase` | `gif`, `ico` and `jpeg` image format plugins |
| `@dx_qt//:plugins_platformthemes` | `qtbase` | `xdgdesktopportal` platform theme plugin |
| `@dx_qt//:plugins_tls` | `qtbase` | `certonly` and `openssl` TLS backend plugins |
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
| `@dx_qt//:cxx_qt` | `qtbase` | CXX-Qt 0.10.0 C++ runtime for `qt_rust_library` |

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

On Linux arm64, use `@rules_dx//qt/platforms:linux_arm64`.

Gate Qt targets to the qualified cells with `QT_COMPATIBLE_WITH` from
`@rules_dx//qt/rules:defs.bzl`. It selects on the os and cpu of each
qualified cell, so the targets build on every qualified cell and are skipped
elsewhere:

```python
load("@rules_dx//qt/rules:defs.bzl", "QT_COMPATIBLE_WITH", "qt_cc_binary")

qt_cc_binary(
    name = "app",
    srcs = ["main.cc"],
    target_compatible_with = QT_COMPATIBLE_WITH,
)
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

`bazel run` and `bazel test` carry only `minimal` and `offscreen`. Any other
plugin fails with `Could not find the Qt platform plugin`. `qt_bundle` ships
`xcb` and `wayland` with `display`. They carry no image format, TLS or theme
plugins either; list `imageformats`, `platformthemes` or `tls` in `plugins` on
`qt_cc_binary`, `qt_cc_test` or `qt_bundle` to ship them; the first two also
put them on `QT_PLUGIN_PATH`. The target's `env`
overrides these variables.

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

## Rust

`qt_rust_library` builds a Rust crate with CXX-Qt bridges and the Qt C++
library generated from them. Load it from `@rules_dx//qt/rules:rust.bzl`:

```python
load("@rules_dx//qt/rules:rust.bzl", "qt_rust_library")

qt_rust_library(
    name = "model",
    srcs = ["lib.rs"],
    bridges = ["ledger_model.rs"],
    crate_root = "lib.rs",
    uri = "Ledger.Model",
    deps = ["//ledger"],
)
```

Each bridge file holds one `#[cxx_qt::bridge]` module. A pinned generator
built from CXX-Qt 0.10.0 and cxx 1.0.202 writes, per bridge,
`<name>.cxxqt.h`, `<name>.cxxqt.cpp`, `<name>.cxx.h` and `<name>.cxx.cpp`
next to the bridge. Include the QObject header as
`"<package>/<name>.cxxqt.h"`. `moc` and `qmltyperegistrar` run on it, so a
`#[qml_element]` type joins the `uri` module.

Attributes:

- `bridges`: one or more `.rs` files with one bridge each. They must sit in
  the target's package directory, not a subdirectory.
- `srcs`, `crate_root`, `crate_name`, `crate_features`, `rustc_flags`, `deps`:
  the Rust crate. It builds as `<name>_rust`, a `rust_static_library` with the
  `cxx` and `cxx-qt` crates added.
- `cxx`, `cxx_qt`: the `cxx` and `cxx-qt` crate targets. They default to the
  pinned crates; point them at your own to build against another version.
- `generator`: the bridge generator executable for `cxx_qt`. It defaults to the
  pinned one built from cxx 1.0.202.
- `cc_deps`: C++ dependencies of the generated library.
- `uri`, `version`, `qml_srcs`, `qml_depends`, `modules`, `qt`, `copts`: as on
  `qt_qml_module`. `modules` defaults to `["qml"]` with a `uri`, else `["core"]`.

`qt_rust_cc_library` from `@rules_dx//qt/rules:defs.bzl` builds only the C++
side for an existing Rust static library, passed as `rust`.

Link one `qt_rust_library` per binary: each carries the Rust standard
library. Put all bridges of an app in it. CXX-Qt types from `cxx-qt-lib`,
such as `QString`, are not available. Use `bool`, integer and float
properties and arguments. Integer properties reach QML as `int`, `uint`,
`short`, `ushort`, `qint8`, `quint8`, `qlonglong` or `qulonglong`. Mark
properties Rust owns `READ, NOTIFY`; QML writes to them then fail. A bridge
error, such as a `NOTIFY` signal that does not exist, fails the
`QtRustBridge` action with `<file>:<line>:<column>`.

The generated C++ and `@dx_qt//:cxx_qt` call `cxxbridge1$202$...` symbols, so
the Rust crate must link `cxx` 1.0.202. Another `cxx` version fails at link
time unless you also pass a matching `generator`.

Builds that use no `qt_rust_library` fetch none of the CXX-Qt crates.

## Packaging

`qt_bundle` stages what a `qt_cc_binary` runs with: its shared libraries
under `lib/`, the fonts under `fonts/` (an app must point `FONTCONFIG_FILE` at `fonts/fonts.conf` itself to use them), the platform plugins under `plugins/platforms/` when it links
`gui`, the `plugins` types under `plugins/<type>/`, the QML import modules under `qml/` and the Qt, ICU and closure
license texts under `licenses/`. `lib/` holds the libraries the
binary links, the libraries those need, and the ones the shipped QML
plugins need. Pass it to `consumer_archive` as a
resource:

```python
load("@rules_dx//deploy/rules:archive.bzl", "consumer_archive")
load("@rules_dx//qt/rules:defs.bzl", "QT_CONSUMER_ARCH", "QT_CONSUMER_MIN_RUNTIME", "qt_bundle")

qt_bundle(
    name = "runtime",
    binary = ":app",
    plugins = ["imageformats"],
)

consumer_archive(
    name = "app_release",
    app = ":app",
    arch = QT_CONSUMER_ARCH,
    min_runtime = QT_CONSUMER_MIN_RUNTIME,
    notice = "NOTICE",
    os = "linux",
    resources = [":runtime"],
)
```

`QT_CONSUMER_ARCH` and `QT_CONSUMER_MIN_RUNTIME` select the archive identity
of the targeted cell: `x86_64` and `glibc-2.34` on `linux_x86_64`, `aarch64`
and `glibc-2.39` on `linux_arm64`.

- `binary`: the `qt_cc_binary`.
- `qml_modules`: import modules to ship. Defaults to `["QML", "QtQml"]` when
  the binary uses `qml`. A module missing from the import tree fails analysis.
- `display`: display platforms to ship, `xcb` and `wayland`. Each adds its
  platform plugin and the libraries it needs. `xcb` also ships the
  `glx` and `egl` GL integration plugins that Qt Quick loads for an OpenGL
  context. Defaults to none.
- `plugins`: plugin types to ship, `imageformats`, `platformthemes` and `tls`.
  Each adds its plugins and the libraries they need. Defaults to none. The
  `tls` OpenSSL backend loads the host's OpenSSL 3 `libssl.so.3` at run time.
- `licenses`: extra license texts to ship, such as `fmt/LICENSE` for a bundled library. Each path needs a `licenses/<name>/` directory and lands under `licenses/<name>/`.
- `qt`: the Qt repo, `@dx_qt` by default.

`qt_cc_binary` links with RPATH `$ORIGIN/resources/lib`, so the extracted
archive finds its libraries from any directory with no environment. Qt finds
`resources/qml` from the location of `libQt6Core`. A `qt.conf` beside the
binary overrides these paths.

## Linking Qt

Build Qt code for a Qt platform, or a platform with the same `@llvm`
constraints:

| Platform | `@llvm` constraints |
| --- | --- |
| `@rules_dx//qt/platforms:linux_x86_64` | `libstdcxx.11.5.0`, `gnu.2.34` |
| `@rules_dx//qt/platforms:linux_arm64` | `libstdcxx.14.3.0`, `gnu.2.39` |

Without the rules, compile with `-fPIC` and link dynamically:

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
and `QT_QPA_PLATFORM_PLUGIN_PATH`. Only glibc comes from the host: 2.34 or
newer on x86_64, 2.39 or newer on arm64.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified: Core, Network, QML, QtTest, Gui, Widgets, Quick and Quick Test on the `offscreen` plugin with software rendering, `moc`, `rcc`, `uic`, `qmltyperegistrar`, `lrelease`, `qmlformat`, `qmllint`, `qt_bundle` archives, `qt_rust_library`. `xcb` and `wayland` plugins in `qt_bundle` archives, loaded without host libraries. Not qualified: a real display, GPU rendering |
| `linux_arm64` | Qualified: Core, Network, QML, QtTest, Gui, Widgets, Quick and Quick Test on the `offscreen` plugin with software rendering, `moc`, `rcc`, `uic`, `qmltyperegistrar`, `lrelease`, `qmlformat`, `qmllint`, `qt_bundle` archives, `qt_rust_library`. `xcb` and `wayland` plugins in `qt_bundle` archives, loaded without host libraries. Not qualified: a real display, GPU rendering |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

Other platforms fail analysis and list the qualified cells. Fetching needs a
Linux x86_64 or Linux arm64 host.
