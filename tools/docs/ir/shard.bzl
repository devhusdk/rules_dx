"""Language-neutral documentation IR shard provider."""

DocsShardInfo = provider(
    doc = "One validated binary documentation IR shard.",
    fields = {
        "language": "Language prefix of every symbol id.",
        "package": "Package segment of every symbol id.",
        "shard": "The binary DocIr shard file.",
    },
)

def srcs_manifest_line(exec_path, rel, generated):
    """Returns one tab-separated srcs manifest line."""
    return exec_path + "\t" + rel + "\t" + ("1" if generated else "0") + "\n"
