"""Fixture tool repositories for the acquisition tests."""

load("//quality/artifacts:acquire.bzl", "ACQUIRE_ATTRS", "acquire_tool")

_HOST_UTILITIES = ["chmod", "python3", "sha256sum", "shasum"]

def _fixture_repo_impl(ctx):
    """Acquires one fixture and refuses a host that still has the old utilities."""
    found = [name for name in _HOST_UTILITIES if ctx.which(name) != None]
    if found != []:
        fail("acquire fixture: the host PATH still has " + ", ".join(found) +
             "; fetch these fixtures with an empty --repo_env PATH")
    acquire_tool(ctx)

acquire_fixture_repo = repository_rule(
    implementation = _fixture_repo_impl,
    attrs = ACQUIRE_ATTRS,
    environ = ["PATH"],
)
