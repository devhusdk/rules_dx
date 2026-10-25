"""Language-neutral documentation IR shard provider."""

DocsShardInfo = provider(
    doc = "One validated binary documentation IR shard.",
    fields = {
        "language": "Language prefix of every symbol id.",
        "package": "Package segment of every symbol id.",
        "shard": "The binary DocIr shard file.",
    },
)
