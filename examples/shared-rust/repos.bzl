"""An independent shared-tasks consumer built on rules_dx public rules."""

_CONSUMER_INDEX = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>dx shared consumer</title>
</head>
<body>
<main>
<h1>dx shared consumer</h1>
<ol id="c-tasks"></ol>
<p id="c-summary">loading</p>
<button id="c-add" type="button">Add consumer task</button>
<p id="c-reply">pending</p>
<p id="c-asset">loading</p>
<p id="c-version">loading</p>
</main>
<script type="module">
import init, { WebBoard, asset_report, core_version } from "./shared_tasks.js";
await init();
const board = new WebBoard(await (await fetch("./data/board.txt")).text());
const render = () => {
  document.getElementById("c-tasks").replaceChildren(...board.lines().map((line) => {
    const item = document.createElement("li");
    item.textContent = line;
    return item;
  }));
  document.getElementById("c-summary").textContent = board.summary();
};
document.getElementById("c-add").addEventListener("click", () => {
  document.getElementById("c-reply").textContent = board.apply("add Consumer browser task");
  render();
});
render();
const notes = new Uint8Array(await (await fetch("./data/notes.txt")).arrayBuffer());
document.getElementById("c-asset").textContent = asset_report("notes.txt", notes);
document.getElementById("c-version").textContent = core_version();
</script>
</body>
</html>
"""

_CONSUMER_BOARD = """# consumer board
[ ] Review the shared core @notes.txt
[x] Wire the consumer app
"""

_CONSUMER_NOTES = "Consumer-owned notes.\n"

_CONSUMER_STRINGS = """<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="app_name">Consumer Tasks</string>
</resources>
"""

_REPORT_RS = """//! Consumer-owned report adapter over the shared task board.

#[cfg(not(consumer_build))]
compile_error!("consumer rustc flags did not reach the adapter");

use std::process::ExitCode;

use shared_tasks::Board;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((path, commands)) = args.split_first() else {
        eprintln!("usage: consumer_report BOARD [COMMAND]...");
        return ExitCode::from(2);
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("err:unreadable: {path}");
        return ExitCode::from(2);
    };
    let mut board = match Board::load(&text) {
        Ok(board) => board,
        Err(error) => {
            eprintln!("err:{}", error.kind());
            return ExitCode::from(2);
        }
    };
    for command in commands {
        if let Err(error) = board.apply(command) {
            eprintln!("err:{}", error.kind());
            return ExitCode::from(1);
        }
    }
    #[cfg(feature = "csv")]
    {
        println!("id,done,title");
        for task in board.tasks() {
            println!("{},{},{}", task.id, task.done, task.title);
        }
    }
    println!("summary {}", board.summary());
    let runtime = std::env::var("CONSUMER_RUNTIME").unwrap_or_else(|_| "default".to_owned());
    println!("runtime={runtime}");
    ExitCode::SUCCESS
}
"""

_WRAPPED_RUNTIME = """#!/bin/sh
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"
subcommand="$1"
shift
exec "$DX_WASI_MANAGED_RUNTIME" "$subcommand" --env CONSUMER_RUNTIME=wrapped "$@"
"""

_BUILD_BAZEL = '''load("@rules_dx//android/packaging:defs.bzl", "rust_android_apk")
load("@rules_dx//examples/shared-rust:release.bzl", "release_artifact")
load("@rules_dx//javascript/rules:web.bzl", "javascript_web_app", "javascript_web_server")
load("@rules_dx//rust/rules:defs.bzl", "rust_binary", "rust_wasi_test")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

package(
    default_testonly = True,
    default_visibility = ["//visibility:public"],
)

javascript_web_app(
    name = "consumer_app",
    html = ":index.html",
    bindgen = "@rules_dx//examples/shared-rust:shared_tasks",
    assets = {
        ":assets/board.txt": "data/board.txt",
        ":assets/notes.txt": "data/notes.txt",
    },
)

javascript_web_server(
    name = "consumer_serve",
    app = ":consumer_app",
    port = 8124,
    headers = {"X-Consumer": "shared-consumer"},
)

rust_binary(
    name = "consumer_report",
    srcs = ["report.rs"],
    crate_features = ["csv"],
    crate_root = "report.rs",
    rustc_flags = ["--cfg=consumer_build"],
    deps = ["@rules_dx//examples/shared-rust:tasks_core"],
)

release_artifact(
    name = "consumer_release",
    target = ":consumer_report",
)

genrule(
    name = "consumer_probe",
    srcs = [":assets/board.txt"],
    tools = [":consumer_report"],
    outs = ["probe.txt"],
    cmd = "$(location :consumer_report) $(location :assets/board.txt) 'add Consumer native task' > $@",
)

sh_binary(
    name = "wrapped_runtime",
    srcs = ["wrapped_runtime.sh"],
)

rust_wasi_test(
    name = "consumer_wasi_test",
    binary = ":consumer_report",
    args = ["/board/board.txt", "done 1"],
    preopens = {"/board": ":assets/board.txt"},
    expected_stdout = "\\n".join([
        "id,done,title",
        "1,true,Review the shared core",
        "2,true,Wire the consumer app",
        "summary tasks=2 done=2 open=0 assets=1",
        "runtime=default",
    ]),
    tags = ["block-network"],
)

rust_wasi_test(
    name = "consumer_wasi_runtime_test",
    binary = ":consumer_report",
    args = ["/board/board.txt"],
    preopens = {"/board": ":assets/board.txt"},
    runtime = ":wrapped_runtime",
    expected_stdout = "\\n".join([
        "id,done,title",
        "1,false,Review the shared core",
        "2,true,Wire the consumer app",
        "summary tasks=2 done=1 open=1 assets=1",
        "runtime=wrapped",
    ]),
    tags = ["block-network"],
)

rust_android_apk(
    name = "consumer_apk",
    app_id = "org.example.consumer.tasks",
    abis = ["x86_64"],
    api_level = 33,
    assets = [
        ":assets/board.txt",
        ":assets/notes.txt",
    ],
    assets_dir = "assets",
    permissions = ["android.permission.VIBRATE"],
    resource_files = [":res/values/strings.xml"],
    version_code = 3,
    version_name = "3.0",
    deps = ["@rules_dx//examples/shared-rust:tasks_activity"],
)
'''

def _shared_consumer_repo_impl(ctx):
    """Writes the applications an independent consumer would own."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("index.html", _CONSUMER_INDEX)
    ctx.file("assets/board.txt", _CONSUMER_BOARD)
    ctx.file("assets/notes.txt", _CONSUMER_NOTES)
    ctx.file("res/values/strings.xml", _CONSUMER_STRINGS)
    ctx.file("report.rs", _REPORT_RS)
    ctx.file("wrapped_runtime.sh", _WRAPPED_RUNTIME, executable = True)
    return ctx.repo_metadata(reproducible = True)

shared_consumer_repo = repository_rule(
    implementation = _shared_consumer_repo_impl,
)
