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

def dx_bench_test(
        name,
        binary_rule,
        runner,
        kwargs,
        tags = None,
        visibility = None,
        binary_kwargs = None,
        build_defaults = None,
        extra_deps = None,
        compatible_with = None,
        runner_kwargs = None,
        pass_binary = False,
        argv = None):
    """Declares a private <name>_bin and the visible bench test that dx bench runs."""
    binary = name + "_bin"
    build, run = split_test_kwargs(kwargs)
    run = with_default_size(run)
    build = dict(build_defaults or {}) | build | dict(binary_kwargs or {})
    if extra_deps:
        build["deps"] = build.get("deps", []) + extra_deps
    if compatible_with:
        run["target_compatible_with"] = (run.get("target_compatible_with") or []) + compatible_with
    binary_rule(
        name = binary,
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
        **build
    )
    args = (["$(rootpath :" + binary + ")"] if pass_binary else []) + (argv or []) + run.pop("args", [])
    if args:
        run["args"] = args
    runner(
        name = name,
        data = run.pop("data", []) + [":" + binary],
        tags = merge_tags(tags, "dx-bench", "manual"),
        testonly = True,
        visibility = visibility,
        **(dict(runner_kwargs or {}) | run)
    )
