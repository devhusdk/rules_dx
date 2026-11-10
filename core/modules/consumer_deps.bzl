"""Exports the consumer bazel_dep floor and tested table as JSON for Rust tests and tools."""

load(":versions.bzl", "CONSUMER_BAZEL_DEPS")

def _consumer_deps_json_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    table = {
        module: {"floor": pins.floor, "tested": pins.tested}
        for module, pins in CONSUMER_BAZEL_DEPS.items()
    }
    ctx.actions.write(out, json.encode_indent(table) + "\n")
    return [DefaultInfo(files = depset([out]), runfiles = ctx.runfiles([out]))]

consumer_deps_json = rule(
    implementation = _consumer_deps_json_impl,
    doc = "Writes CONSUMER_BAZEL_DEPS as {module: {floor, tested}} JSON.",
)
