"""Stages conflicting node runtimes for the launcher ambiguity test."""

def _ambiguous_nodes_impl(ctx):
    """Writes two node runtimes into one runfiles closure."""
    runtimes = []
    for name in ["node_a/node.exe", "node_b/node.exe"]:
        runtime = ctx.actions.declare_file("ambiguous_node/" + name)
        ctx.actions.write(runtime, "runtime\n")
        runtimes.append(runtime)
    return [DefaultInfo(
        files = depset(runtimes),
        runfiles = ctx.runfiles(files = runtimes),
        executable = runtimes[0],
    )]

ambiguous_nodes = rule(
    doc = "Stages two node runtimes in one runfiles closure.",
    implementation = _ambiguous_nodes_impl,
)
