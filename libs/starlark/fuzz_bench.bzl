"""Shared argument handling for the fuzz and bench macros."""

DEFAULT_SIZE = "small"

_TEST_KWARGS = ("size", "timeout", "env", "args", "flaky", "shard_count", "target_compatible_with", "data", "local")
_RUNNER_KWARGS = ("args", "target_compatible_with")

def split_test_kwargs(kwargs, keys = _TEST_KWARGS):
    """Returns (build, run): `run` holds the test-shaped kwargs the visible target takes, `build` the rest."""
    build = {key: value for key, value in kwargs.items() if key not in keys and key not in ("tags", "testonly")}
    run = {key: value for key, value in kwargs.items() if key in keys}
    return build, run

def split_runner_kwargs(kwargs):
    """Like split_test_kwargs for a visible target that is not a test rule."""
    return split_test_kwargs(kwargs, _RUNNER_KWARGS)

def with_default_size(run):
    """Returns `run` with the shared default test size."""
    return dict(run, size = run.get("size", DEFAULT_SIZE))

def merge_tags(tags, *extra):
    """Returns `tags` followed by each of `extra` that is not already there."""
    out = list(tags or [])
    for tag in extra:
        if tag not in out:
            out.append(tag)
    return out
