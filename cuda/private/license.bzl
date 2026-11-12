"""Repository rule that stands in for a CUDA component until the EULA is accepted."""

def _cuda_unaccepted_impl(repository_ctx):
    fail(repository_ctx.attr.error)

cuda_unaccepted = repository_rule(
    implementation = _cuda_unaccepted_impl,
    attrs = {
        "error": attr.string(mandatory = True, doc = "Message the fetch fails with."),
    },
    doc = "Fails every fetch with the unaccepted-license message.",
)
