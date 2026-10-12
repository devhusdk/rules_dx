"""Shared Jest and plain-Node test helpers for the JavaScript family."""

load("//libs/starlark:wrapper.bzl", "dx_forwarded_optional", "dx_quality_sources", "dx_symlink_default_info")

JEST_ONLY_KEYS = [
    "node_modules",
    "config",
    "snapshots",
    "run_in_band",
    "colors",
    "auto_configure_reporters",
    "auto_configure_test_sequencer",
    "snapshots_ext",
    "quiet_snapshot_updates",
]

def jest_reporter_rejection(jest_rule, plain_rule, kwargs):
    """Returns the rejection for a disabled standard reporter setup, or None."""
    if kwargs.get("auto_configure_reporters", True) == False:
        return (jest_rule + " always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported. " +
                "Use " + plain_rule + " for a custom runner.")
    return None

def jest_plain_rejection(jest_rule, plain_rule, kwargs):
    """Returns the rejection for a Jest-only kwarg on a plain runner, or None."""
    for key in JEST_ONLY_KEYS:
        if key in kwargs:
            return (plain_rule + " runs plain Node via js_test without " +
                    "Jest reporting; `" + key + "` is not supported. " +
                    "Use " + jest_rule + " for Jest behavior.")
    return None

def jest_test_env(env_inherit):
    """Computes the effective test-runtime inherited environment."""
    env = list(env_inherit) if env_inherit != None else []
    if "TESTBRIDGE_TEST_ONLY" not in env:
        env.append("TESTBRIDGE_TEST_ONLY")
    return env

def jest_upstream_kwargs(kwargs, tags):
    """Returns upstream kwargs with aspect hints dropped and a manual tag."""
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    if tags != None:
        upstream_tags = list(tags)
        if "manual" not in upstream_tags:
            upstream_tags.append("manual")
        upstream_kwargs["tags"] = upstream_tags
    elif upstream_kwargs.get("tags", None) == None:
        upstream_kwargs["tags"] = ["manual"]
    elif "manual" not in upstream_kwargs["tags"]:
        upstream_kwargs["tags"] = upstream_kwargs["tags"] + ["manual"]
    return upstream_kwargs

def jest_data_with_package_json(data):
    """Returns data with the package manifest included."""
    out = list(data)
    if "//:package_json" not in out:
        out.append("//:package_json")
    return out

def jest_test_forward(ctx, what, specs):
    """Returns the forwarding providers for one Jest family test."""
    upstream = ctx.attr.upstream
    env_inherit = list(ctx.attr.env_inherit) if ctx.attr.env_inherit else []
    if "TESTBRIDGE_TEST_ONLY" not in env_inherit:
        env_inherit.append("TESTBRIDGE_TEST_ONLY")
    out = [
        dx_symlink_default_info(ctx, what),
        testing.TestEnvironment({}, env_inherit),
        dx_quality_sources(ctx.files.srcs, specs, str(ctx.label)),
    ]
    return out + dx_forwarded_optional(upstream, [InstrumentedFilesInfo, OutputGroupInfo], what)
