# Qt tasks example

A Qt 6 task browser. Four QML modules, C++ models, a Qt Quick board, a pinned
`fmt` dependency, a German translation and JSON storage. It builds against the
pinned `@dx_qt` SDK on Linux x86_64 and packages into relocatable archives.

| Package | Role |
| --- | --- |
| `store` | `Tasks.Store`: `TaskModel`, a list model with signals and JSON load and save |
| `catalog` | `Tasks.Catalog`: `AssetCatalog`, read from the embedded `assets.json` resource |
| `browser` | `Tasks.Browser`: `Main.qml` drives both models; `tasks_de.ts` compiles to `:/i18n/tasks_de.qm` |
| `board` | `Tasks.Board`: `Board.qml` and `TaskRow.qml`, a Qt Quick list of tasks with click and keyboard toggling |
| `app` | `tasks` and `board` binaries, `browser_test` QtTest suite, `board_quick_test` Qt Quick Test suite, `runtime` and `board_runtime` bundles, `tasks_release` and `board_release` archives |

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

`board --store <path> <image>` renders the task board to an image, such as
`board.png` or `board.ppm`, and prints `<image> <width>x<height> <platform>`.
It uses the `offscreen` platform and software rendering unless
`QT_QPA_PLATFORM` is set.

```sh
bazel run //examples/qt-tasks/app:board -- --store /tmp/tasks.json /tmp/board.png
```

Board exit codes: `0` success, `1` the QML module failed to load, `2` usage
error, `3` storage read failure, `4` image write failure.

`board_quick_test` runs `tst_board.qml`. It clicks the boxes, moves the
highlight with the arrow keys, toggles with Space, checks the rendered pixels
and reloads the saved JSON from a temporary directory.

## Package

```sh
bazel build //examples/qt-tasks/app:tasks_release_archive
mkdir /tmp/tasks && tar -xzf bazel-bin/examples/qt-tasks/app/tasks_release.tar.gz -C /tmp/tasks
env -i /tmp/tasks/tasks --store /tmp/tasks/tasks.json list
```

The archive holds `tasks`, `NOTICE`, `IDENTITY.txt` and `resources/` with
the libraries `tasks` links and needs, the `QML` and `QtQml` import modules and
the license texts for Qt, `fmt` and LLVM. Only glibc 2.34 or newer comes from the host.

```sh
bazel build //examples/qt-tasks/app:board_release_archive
mkdir /tmp/board && tar -xzf bazel-bin/examples/qt-tasks/app/board_release.tar.gz -C /tmp/board
env -i /tmp/board/board --store /tmp/tasks.json /tmp/board/board.png
```

The board archive also holds the `minimal` and `offscreen` platform plugins
under `resources/plugins/platforms/` and the `QtQuick` import modules. Fonts
come from the host fontconfig setup.
