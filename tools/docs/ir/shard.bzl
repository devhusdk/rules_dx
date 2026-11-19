"""Language-neutral documentation IR shard provider."""

load("//core:rlocation.bzl", "repo_relative_path")

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

def srcs_manifest_row(file):
    """Returns the srcs manifest row of one file without its newline, for Args.add_all map_each."""
    return srcs_manifest_line(file.path, repo_relative_path(file.short_path), not file.is_source)[:-1]
