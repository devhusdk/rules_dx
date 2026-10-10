"""An independent shared-core consumer built on rules_dx public rules."""

_CONSUMER_INDEX = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>dx shared consumer</title>
</head>
<body>
<main>
<h1>dx shared consumer</h1>
<p id="c-proof">loading</p>
<p id="c-failure">pending</p>
<p id="c-version">loading</p>
<p id="c-asset">loading</p>
</main>
<script type="module">
import init, { evaluate, core_version } from "./shared_core.js";
await init();
document.getElementById("c-proof").textContent = evaluate("(10 - 4) / 3");
document.getElementById("c-failure").textContent = evaluate("5 % 0");
document.getElementById("c-version").textContent = core_version();
const asset = await (await fetch("./assets/queries.txt")).text();
document.getElementById("c-asset").textContent = asset.trim();
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
    bindgen = "@rules_dx//examples/shared-rust:shared_core",
    assets = {":data.txt": "assets/queries.txt"},
)

javascript_web_server(
    name = "consumer_serve",
    app = ":consumer_app",
    port = 8124,
    headers = {"X-Consumer": "shared-consumer"},
)

genrule(
    name = "consumer_probe",
    tools = ["@rules_dx//examples/shared-rust:core_bin"],
    outs = ["probe.txt"],
    cmd = "$(location @rules_dx//examples/shared-rust:core_bin) '2 * (3 + 4)' > $@",
)
'''

def _shared_consumer_repo_impl(ctx):
    """Writes the browser application an independent consumer would own."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("index.html", _CONSUMER_INDEX)
    ctx.file("data.txt", "1 + 2 * 3\n")

shared_consumer_repo = repository_rule(
    implementation = _shared_consumer_repo_impl,
)
