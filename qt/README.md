# Qt

This directory helps you build Qt apps with a pinned Qt SDK with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//qt:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Qt page](../docs/languages/qt.md) shows
setup, every rule and the tools dx runs.

Run the Qt tests from a rules_dx checkout:

```sh
tools/dx test //qt/...
```

## Quality

Format your QML files with the pinned `qmlformat`. `dx format` runs
`@dx_qt//:qmlformat` when a `qml` family selects `qmlformat`:

```sh
dx format --apply -- --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat
```

## Packaging

Ship a Qt app as an archive. `qt_bundle` collects what a `qt_cc_binary`
needs at run time, and `qt_consumer_archive` packs it with the app.
`qt_consumer_archive` and `qt_bundle_runtime` load from
`@rules_dx//deploy:defs.bzl`:

```python
load("@rules_dx//deploy:defs.bzl", "qt_bundle_runtime", "qt_consumer_archive")
load("@rules_dx//qt:defs.bzl", "qt_bundle")

qt_bundle(
    name = "runtime",
    binary = ":app",
    plugins = ["imageformats"],
)

qt_bundle_runtime(
    name = "runtime_runtime",
    bundle = ":runtime",
)

qt_consumer_archive(
    name = "app_release",
    app = ":app",
    notice = "NOTICE",
    resources = [":runtime"],
    runtime = [":runtime_runtime"],
)
```

`qt_bundle` stages:

- under `lib/`: the libraries the binary links, the libraries those
  need, and the ones the shipped QML plugins need;
- under `fonts/`: the fonts. To use them, your app must point
  `FONTCONFIG_FILE` at `fonts/fonts.conf` itself;
- under `plugins/platforms/`: the platform plugins, when the binary
  links `gui`;
- under `plugins/<type>/`: the `plugins` types;
- under `qml/`: the QML import modules;
- under `licenses/`: the Qt, ICU and closure license texts.

`qt_bundle` is skipped on cells without a needs table, such as the
Windows cells. A cell is one platform's Qt archives and runtime closure.
On macOS, any non-empty `display` ships the `cocoa` platform plugin.

`qt_bundle` takes:

- `binary`: the `qt_cc_binary`.
- `qml_modules`: import modules to ship, none by default. A module missing from the import tree fails analysis.
- `display`: display platforms to ship, `xcb` and `wayland`. Each adds its
  platform plugin and the libraries it needs. `xcb` also ships the
  `glx` and `egl` GL integration plugins that Qt Quick loads for an OpenGL
  context. `wayland` likewise ships the `wayland-egl` graphics integration
  plugin and `libwayland-egl`. Defaults to none.
- `plugins`: plugin types to ship, `imageformats`, `platformthemes` and `tls`.
  Each adds its plugins and the libraries they need. Defaults to none. The
  `tls` OpenSSL backend loads the host's OpenSSL 3 `libssl.so.3` at run time.
- `licenses`: extra license texts to ship, such as `fmt/LICENSE` for a
  bundled library. Each path needs a `licenses/<name>/` directory and
  lands under `licenses/<name>/`.
- `qt`: the Qt repo, `@dx_qt` by default.

`qt_bundle_runtime` holds the Windows DLLs of a `qt_bundle` and a
`qt.conf` that points Qt at `resources/`, so `consumer_archive` puts them
next to the executable. It ships nothing on cells without DLLs.

`qt_consumer_archive` takes the arguments of `consumer_archive`. It
records the os and cpu of the target platform, and `runtime` gains the
OS runtime of the targeted cell. `min_runtime` defaults to the minimum
runtime of the targeted cell: `glibc-2.34` on `linux_x86_64`,
`glibc-2.39` on `linux_arm64` and `macos-14.4` on `macos_arm64`.

`qt_cc_binary` links with RPATH `$ORIGIN/resources/lib`, so the extracted
archive finds its libraries from any directory, with no environment
variables set. Qt finds
`resources/qml` from the location of `libQt6Core`. A `qt.conf` beside the
binary overrides these paths.

## Toolchain

### Setup

Adding `rules_dx` is all you need. Every Qt module (`qtbase`,
`qtdeclarative`, `qttools`) is declared, and Bazel fetches nothing until a
target needs it. `qttools` carries only `lrelease`. To download from a
mirror, call `configure` in your root `MODULE.bazel`:

```python
qt = use_extension("@rules_dx//qt:extensions.bzl", "qt")
qt.configure(
    mirror = "https://mirror.example/qtsdkrepository",
)
```

`configure` takes:

- `version`: must be the pinned Qt version. Other versions fail.
- `mirror`: an https mirror of `download.qt.io/online/qtsdkrepository`.
- `runtime_mirror`: an https mirror of `vault.almalinux.org`, with its
  `9.6/` and `10.1/` `BaseOS/` and `AppStream/` trees.

`configure` is optional, works only in the root module, and is called at
most once.

### Your own cell

Declare your own cell with `qt.cell` in your root `MODULE.bazel`:

```python
qt.cell(
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

- `name`: lowercase letters, digits and `_`. Each Qt module of the cell
  (`qtbase`, `qtdeclarative`, `qttools`) is fetched into
  `@dx_qt_<name>_<module>` when a target needs it, and `@dx_qt//:core` and
  the other targets route to the module that owns them.
  A pinned cell of the same name (`linux_x86_64`, `linux_arm64`,
  `macos_arm64`, `windows_arm64`, `windows_x86_64`) is replaced.
- `constraints`: the platform constraints the cell serves. They must differ
  from every other cell's.
- `layout`: the file layout of the archives. Only `linux` (the default).
- `spec`: a `.json` file with `archives`, `directory`, `release` and
  `runtime`, and optionally `needs`: the `library`, `plugin` and
  `runtime_plugin` tables of what each shared object needs at run time.

Start from a pinned spec. This copies the `linux_x86_64` spec into your
repo:

```sh
bazel build @rules_dx//qt:linux_x86_64_cell
cp "$(bazel cquery --output=files @rules_dx//qt:linux_x86_64_cell)" third_party/qt/custom_x86_64.json
```

The spec is checked like the pinned cells. Every archive and runtime package
needs a sha256. Archives come from `mirror` (or `download.qt.io`) under
`directory`. Runtime packages come from `runtime_mirror` (or
`vault.almalinux.org`). The Qt release must be 6.12.0. Unknown keys fail.

Only the root module may declare cells. `qt_bundle` packages a cell only
when its spec carries `needs`, and those tables must match its archives.
Without them, `@dx_qt//:needs` fails analysis, the step where Bazel reads
BUILD files before it runs anything, and says why.

### Targets

| Target | Module | What it is |
| --- | --- | --- |
| `@dx_qt//:core` | `qtbase` | QtCore headers and library |
| `@dx_qt//:network` | `qtbase` | QtNetwork headers and library |
| `@dx_qt//:gui` | `qtbase` | QtGui headers and library |
| `@dx_qt//:widgets` | `qtbase` | QtWidgets headers and library |
| `@dx_qt//:test` | `qtbase` | QtTest headers and library |
| `@dx_qt//:fonts` | `qtbase` | `fonts.conf` and the DejaVu Sans fonts |
| `@dx_qt//:platforms` | `qtbase` | `minimal` and `offscreen` platform plugins |
| `@dx_qt//:display_cocoa` | `qtbase` | `cocoa` platform plugin, macOS only |
| `@dx_qt//:display_wayland` | `qtbase` | `wayland` platform plugin, `xdg-shell`, `bradient` and `wayland-egl` plugins |
| `@dx_qt//:display_xcb` | `qtbase` | `xcb` platform plugin and its `glx` and `egl` GL integration plugins |
| `@dx_qt//:plugins_imageformats` | `qtbase` | `gif`, `ico` and `jpeg` image format plugins |
| `@dx_qt//:plugins_platformthemes` | `qtbase` | `xdgdesktopportal` platform theme plugin |
| `@dx_qt//:plugins_tls` | `qtbase` | `certonly` and `openssl` TLS backend plugins |
| `@dx_qt//:metatypes` | `qtdeclarative` | Qt module metatypes JSON |
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
| `@dx_qt//:runtime` | every module | Qt, ICU, closure and graphics stack shared libraries |
| `@dx_qt//:licenses` | every module | Qt, ICU and closure license texts |
| `@dx_qt//:sbom` | every module | Qt SPDX documents |
| `@dx_qt//:cxx_qt` | `qtbase` | CXX-Qt 0.10.0 C++ runtime for `qt_rust_library` |

A target fetches the repository of the module that owns it, and of the
modules that module requires. Each Qt library exists once, in the
repository of its module. The other module repositories link to it.

### Gui, Widgets and Quick

A `qt_cc_binary` or `qt_cc_test` that links `gui` runs with the pinned
fonts and platform plugins: `bazel run` and `bazel test` set
`FONTCONFIG_PATH` and `QT_QPA_PLATFORM_PLUGIN_PATH`. Tests also set
`QT_QPA_PLATFORM=offscreen` and `QT_QUICK_BACKEND=software`, so they need no
display or GPU. For `bazel run`, pick the plugin yourself:

```sh
QT_QPA_PLATFORM=offscreen bazel run //app:viewer
```

`bazel run` and `bazel test` carry only the `minimal` and `offscreen`
platform plugins. Any other platform plugin fails with
`Could not find the Qt platform plugin`. `qt_bundle` ships `xcb` and
`wayland` through `display`.

They carry no image format, TLS or theme plugins either. To ship them,
list `imageformats`, `platformthemes` or `tls` in `plugins` on
`qt_cc_binary`, `qt_cc_test` or `qt_bundle`. On `qt_cc_binary` and
`qt_cc_test`, this also puts them on `QT_PLUGIN_PATH`. The target's `env`
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

The last argument is the test directory, relative to the workspace root.

`dx lint` and `dx format` see only the hand-written C, C++ and QML files.
Generated `moc`, `uic`, `rcc` and registration sources are never checked.

### Rust

`qt_rust_library` builds a Rust crate with CXX-Qt bridges and the Qt C++
library generated from them. Load it from `@rules_dx//qt:defs.bzl`:

```python
load("@rules_dx//qt:defs.bzl", "qt_rust_library")

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

`qt_rust_cc_library` from `@rules_dx//qt:defs.bzl` builds only the C++
side for an existing Rust static library, passed as `rust`.

Rules for bridges:

- Link one `qt_rust_library` per binary, since each carries the Rust
  standard library. Put all bridges of an app in it.
- CXX-Qt types from `cxx-qt-lib`, such as `QString`, are not available.
  Use `bool`, integer and float properties and arguments.
- Integer properties reach QML as `int`, `uint`, `short`, `ushort`,
  `qint8`, `quint8`, `qlonglong` or `qulonglong`.
- Mark properties that Rust owns `READ, NOTIFY`. QML writes to them then
  fail.
- A bridge error, such as a `NOTIFY` signal that does not exist, fails
  the `QtRustBridge` action with `<file>:<line>:<column>`.

The generated C++ and `@dx_qt//:cxx_qt` call `cxxbridge1$202$...` symbols, so
the Rust crate must link `cxx` 1.0.202. Another `cxx` version fails at link
time unless you also pass a matching `generator`.

Builds that use no `qt_rust_library` fetch none of the CXX-Qt crates.

### Linking Qt

Build Qt code for a Qt platform, or for a platform with the same `@llvm`
constraints:

| Platform | `@llvm` constraints |
| --- | --- |
| `@rules_dx//qt:linux_x86_64` | `libstdcxx.11.5.0`, `gnu.2.34` |
| `@rules_dx//qt:linux_arm64` | `libstdcxx.14.3.0`, `gnu.2.39` |

To use plain `cc_binary` instead of the `qt_` rules, compile with
`-fPIC` and link dynamically:

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

### Toolchain

rules_dx declares the Qt SDK and registers its toolchains. You add no
`register_toolchains` line. The one tag you may add is `qt.configure`.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).

`qt_cc_binary`, `qt_cc_test` and `qt_bundle` take `platforms`. On a host
build, they build for the Qt platform of the host, with
`--//cc:compiler=msvc` on Windows. Other Qt targets on Windows need
`--//cc:compiler=msvc`. Without it, they are incompatible and skipped.
Other platforms fail analysis and list the supported cells.

A real display and GPU rendering are not supported. Fetching needs a
Linux x86_64 or Linux arm64 host.
