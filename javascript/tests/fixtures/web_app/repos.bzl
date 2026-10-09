"""A consumer-owned browser application built on rules_dx web rules."""

_CONSUMER_INDEX = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>dx consumer web</title>
</head>
<body>
<main>
<h1>dx consumer web</h1>
<p id="consumer-proof">loading</p>
<p id="consumer-asset">loading</p>
</main>
<script type="module">
import init, { greet } from "./wasm_hello_web.js";
await init();
document.getElementById("consumer-proof").textContent = greet("consumer");
const asset = await (await fetch("./assets/data.txt")).text();
document.getElementById("consumer-asset").textContent = asset.trim();
</script>
</body>
</html>
"""

_BUILD_BAZEL = '''load("@rules_dx//javascript/rules:web.bzl", "javascript_web_app", "javascript_web_server")

package(
    default_testonly = True,
    default_visibility = ["//visibility:public"],
)

javascript_web_app(
    name = "consumer_app",
    html = ":index.html",
    bindgen = "@rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_web",
    assets = {":data.txt": "assets/data.txt"},
)

javascript_web_server(
    name = "consumer_serve",
    app = ":consumer_app",
    port = 8123,
    headers = {"X-Consumer": "web-consumer"},
)
'''

def _web_consumer_repo_impl(ctx):
    """Writes the browser application an independent consumer would own."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("index.html", _CONSUMER_INDEX)
    ctx.file("data.txt", "consumer asset\n")

web_consumer_repo = repository_rule(
    implementation = _web_consumer_repo_impl,
)
