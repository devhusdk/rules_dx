# Qt app example

A headless Qt 6 app with a QML module, a C++ QML type, a Qt resource and a
QtTest suite. It builds against the pinned `@dx_qt` SDK on Linux x86_64.

| Package | Role |
| --- | --- |
| `tally` | `qt_qml_module` with URI `Tally`: `Counter` C++ type, `Main.qml` and `Badge.qml` |
| `app` | `tally` binary and `counter_test` QtTest suite |

```sh
bazel build //examples/qt-app/...
bazel test //examples/qt-app/...
bazel run //examples/qt-app/app:tally -- 3
```

The run prints the `banner.txt` resource, loads `Tally/Main`, bumps the
counter three times and prints `tally 46`.

`counter_test` checks the C++ type, evaluates the `Tally` module from its
embedded resources and loads `QtQml.Models` from the pinned import tree.
