"""Stages conflicting node runtimes for the launcher ambiguity test."""

def _ambiguous_nodes_impl(ctx):
    """Writes two node runtimes and one entry stub into one runfiles closure."""
    runtimes = []
    for name in ["node_a/node.exe", "node_b/node.exe"]:
        runtime = ctx.actions.declare_file("ambiguous_node/" + name)
        ctx.actions.write(runtime, "runtime\n")
        runtimes.append(runtime)
    entry = ctx.actions.declare_file("ambiguous_node/entry.js")
    ctx.actions.write(entry, "entry\n")
    return [DefaultInfo(
        files = depset([entry]),
        runfiles = ctx.runfiles(files = [entry] + runtimes),
        executable = runtimes[0],
    )]

ambiguous_nodes = rule(
    doc = "Stages two node runtimes in one runfiles closure.",
    implementation = _ambiguous_nodes_impl,
)
