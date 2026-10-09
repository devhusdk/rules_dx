"""A consumer-owned asset repository with its own resource processor."""

_BUILD_BAZEL = '''load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

exports_files(["banner.txt"])

sh_binary(
    name = "processor",
    srcs = ["processor.sh"],
    visibility = ["//visibility:public"],
)
'''

_BANNER_TXT = """dx-consumer-shared-marker
banner-v1
"""

_PROCESSOR_SH = """#!/bin/sh
set -eu
while [ "$#" -gt 2 ]; do shift; done
in="$1"
out="$2"
{ cat "$in"; printf 'FOOTER-dx-consumer-v1\\n'; } > "$out"
"""

def _consumer_resources_repo_impl(ctx):
    """Writes the asset and processor files a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("banner.txt", _BANNER_TXT)
    ctx.file("processor.sh", _PROCESSOR_SH)

consumer_resources_repo = repository_rule(
    implementation = _consumer_resources_repo_impl,
)
