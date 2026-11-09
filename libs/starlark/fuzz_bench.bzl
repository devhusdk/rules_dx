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
    """Declares a private `<name>_bin` and the visible `<name>` bench test that `dx bench` runs.

    Args:
      name: the visible test target.
      binary_rule: macro or rule called as `binary_rule(name = "<name>_bin", ...)`.
      runner: `native_test` or `sh_test`; it gets `runner_kwargs`.
      kwargs: the caller's remaining kwargs, split into binary and test attrs.
      tags: caller tags; the binary gets "manual", the test "dx-bench" and "manual".
      visibility: visibility of the visible test.
      binary_kwargs: attrs forced on the binary (srcs, deps).
      build_defaults: binary attrs the caller's kwargs may override.
      extra_deps: labels appended to the binary's deps.
      compatible_with: constraints appended to the test's target_compatible_with.
      runner_kwargs: attrs for the visible test, such as `src` or `srcs`.
      pass_binary: put the binary's rootpath first in the test args.
      argv: args placed before the caller's args.
    """
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
