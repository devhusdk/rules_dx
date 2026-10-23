# Qt tasks example

A headless Qt 6 task browser. Three QML modules, C++ models, a pinned `fmt`
dependency, a German translation and JSON storage. It builds against the
pinned `@dx_qt` SDK on Linux x86_64 and packages into a relocatable archive.

| Package | Role |
| --- | --- |
| `store` | `Tasks.Store`: `TaskModel`, a list model with signals and JSON load and save |
| `catalog` | `Tasks.Catalog`: `AssetCatalog`, read from the embedded `assets.json` resource |
| `browser` | `Tasks.Browser`: `Main.qml` drives both models; `tasks_de.ts` compiles to `:/i18n/tasks_de.qm` |
| `app` | `tasks` binary, `browser_test` QtTest suite, `runtime` bundle and `tasks_release` archive |

```sh
bazel build //examples/qt-tasks/...
bazel test //examples/qt-tasks/...
bazel run //examples/qt-tasks/app:tasks -- --store /tmp/tasks.json add "Draw logo" logo
bazel run //examples/qt-tasks/app:tasks -- --lang de --store /tmp/tasks.json list
```

`tasks` takes `[--lang <code>] --store <path>` and one command:

- `add <title> <asset>` adds a task for a catalog asset.
- `done <row>` toggles a task.
- `list` prints every task.

Each command prints the summary line, such as `1 of 2 tasks open`.

Exit codes: `0` success, `1` the QML module failed to load, `2` usage error,
unknown asset or row, missing translation or missing catalog resource, `3`
storage read or write failure.

## Package

```sh
bazel build //examples/qt-tasks/app:tasks_release_archive
mkdir /tmp/tasks && tar -xzf bazel-bin/examples/qt-tasks/app/tasks_release.tar.gz -C /tmp/tasks
env -i /tmp/tasks/tasks --store /tmp/tasks/tasks.json list
```

The archive holds `tasks`, `NOTICE`, `IDENTITY.txt` and `resources/` with
the Qt, ICU and closure libraries, the `QML` and `QtQml` import modules and
their license texts. Only glibc 2.34 or newer comes from the host.
