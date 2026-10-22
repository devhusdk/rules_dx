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

- `modules`: `qtbase` (default) or `qtdeclarative`, which adds `qtbase`.
- `version`: must be `6.11.3`. Other versions fail.
- `mirror`: an https mirror of `download.qt.io/online/qtsdkrepository`.
- `runtime_mirror`: an https mirror of the AlmaLinux 9.6 BaseOS packages.

Call `configure` once.

## Targets

| Target | Module | What it is |
| --- | --- | --- |
| `@dx_qt//:core` | `qtbase` | QtCore headers and library |
| `@dx_qt//:network` | `qtbase` | QtNetwork headers and library |
| `@dx_qt//:qml` | `qtdeclarative` | QtQml headers and library |
| `@dx_qt//:qml_imports` | `qtdeclarative` | QML import tree |
| `@dx_qt//:qtqml_qmldir` | `qtdeclarative` | `qml/QtQml/qmldir`, two levels below the import root |
| `@dx_qt//:qmlformat` | `qtdeclarative` | `qmlformat` launcher |
| `@dx_qt//:qmllint` | `qtdeclarative` | `qmllint` launcher |
| `@dx_qt//:runtime` | any | Qt, ICU and closure shared libraries |
| `@dx_qt//:licenses` | any | Qt, ICU and closure license texts |
| `@dx_qt//:sbom` | any | Qt SPDX documents |

A target from a module you did not enable fails analysis and names the
module. `gui`, `quick` and `widgets` always fail analysis: the closure
carries no graphics stack.

## Formatting QML

`dx format` runs `@dx_qt//:qmlformat` when a `qml` family selects
`qmlformat`:

```sh
dx format --apply -- --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat
```

## Linking Qt

Build Qt code for `@rules_dx//qt/platforms:linux_x86_64`, or a platform
with the same `@llvm` constraints (`libstdcxx.11.5.0`, `gnu.2.34`). Compile
with `-fPIC` and link dynamically:

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
| `linux_x86_64` | Qualified: Core, Network, QML headless, `qmlformat`, `qmllint` |
| `linux_arm64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

Other platforms fail analysis and list the qualified cell. Fetching needs a
Linux x86_64 host.
