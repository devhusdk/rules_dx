"""An independent consumer of the shared Rust core application."""

_CONSUMER_INDEX = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>dx shared consumer</title>
</head>
<body>
<main>
<h1>dx shared consumer</h1>
<p id="consumer-proof">loading</p>
<p id="consumer-count">loading</p>
<p id="consumer-asset">loading</p>
</main>
<script type="module">
import init, { greet, Counter } from "./shared_web.js";
await init();
document.getElementById("consumer-proof").textContent = greet("consumer");
const counter = new Counter();
counter.add(20);
counter.add(22);
document.getElementById("consumer-count").textContent = counter.render();
const asset = await (await fetch("./assets/data.txt")).text();
document.getElementById("consumer-asset").textContent = asset.trim();
</script>
</body>
</html>
"""

_CONSUMER_MAIN = """fn main() {
    println!("{}", shared_app::greeting("consumer"));
}
"""

_CONSUMER_BUILD = '''load("@rules_dx//javascript/rules:web.bzl", "javascript_web_app", "javascript_web_server")
load("@rules_dx//rust/rules:defs.bzl", "rust_binary")

package(
    default_testonly = True,
    default_visibility = ["//visibility:public"],
)

javascript_web_app(
    name = "consumer_app",
    assets = {":data.txt": "assets/data.txt"},
    bindgen = "@rules_dx//rust/tests/fixtures/shared_app:shared_web",
    html = ":index.html",
)

javascript_web_server(
    name = "consumer_serve",
    app = ":consumer_app",
    headers = {"X-Consumer": "shared-consumer"},
    port = 8124,
)

rust_binary(
    name = "consumer_native",
    srcs = ["main.rs"],
    crate_name = "consumer_native",
    crate_root = "main.rs",
    edition = "2021",
    deps = ["@rules_dx//rust/tests/fixtures/shared_app:shared_core"],
)
'''

def _shared_app_consumer_repo_impl(ctx):
    """Writes the browser and native products an independent consumer would own."""
    ctx.file("BUILD.bazel", _CONSUMER_BUILD)
    ctx.file("index.html", _CONSUMER_INDEX)
    ctx.file("data.txt", "consumer asset\n")
    ctx.file("main.rs", _CONSUMER_MAIN)

shared_app_consumer_repo = repository_rule(
    implementation = _shared_app_consumer_repo_impl,
)
