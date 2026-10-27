"""Shared Jest and plain Node test core for the JavaScript and TypeScript wrappers."""

load("//libs/starlark:wrapper.bzl", "dx_args_label_error", "dx_forward_attrs", "dx_forward_data_attr", "dx_forwarded_quiet", "dx_forwarded_run_environment", "dx_lcov_merger_attr", "dx_quality_sources", "dx_symlink_default_info", "dx_test_forward_kwargs", "dx_test_upstream_kwargs")
load("//quality:sources.bzl", "QualitySourcesInfo")

_JEST_ONLY_KEYS = [
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

def node_test_reporter_rejection(jest_rule, plain_rule, kwargs):
    """Returns the rejection for forbidden Jest test kwargs, or None."""
    if kwargs.get("auto_configure_reporters", True) == False:
        return (jest_rule + " always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported. " +
                "Use " + plain_rule + " for a custom runner.")
    return None

def node_test_plain_rejection(jest_rule, plain_rule, kwargs):
    """Returns the rejection for forbidden plain Node test kwargs, or None."""
    for key in _JEST_ONLY_KEYS:
        if key in kwargs:
            return (plain_rule + " runs plain Node via js_test without " +
                    "Jest reporting; `" + key + "` is not supported. " +
                    "Use " + jest_rule + " for Jest behavior.")
    return None

def node_test_env(env_inherit):
    """Computes the effective test-runtime inherited environment."""
    env = list(env_inherit) if env_inherit != None else []
    if "TESTBRIDGE_TEST_ONLY" not in env:
        env.append("TESTBRIDGE_TEST_ONLY")
    return env

def node_test_forward_rule(what, allow_files, quality_specs):
    """Creates the test forwarder rule for one Node test wrapper family."""

    def _impl(ctx):
        return [
            dx_symlink_default_info(ctx, what),
            dx_forwarded_run_environment(ctx.attr.upstream, {}, node_test_env(ctx.attr.env_inherit)),
            dx_quality_sources(ctx.files.srcs, quality_specs, str(ctx.label)),
        ] + dx_forwarded_quiet(ctx.attr.upstream, [InstrumentedFilesInfo, OutputGroupInfo])

    return rule(
        implementation = _impl,
        test = True,
        provides = [DefaultInfo, RunEnvironmentInfo, QualitySourcesInfo],
        attrs = dx_forward_attrs(
            allow_files = allow_files,
            upstream_providers = [[DefaultInfo]],
            extra_attrs = {
                "env_inherit": attr.string_list(),
            } | dx_lcov_merger_attr() | dx_forward_data_attr(),
        ),
    )

def node_test_upstream_data(upstream_data, package_json, label_key):
    """Returns upstream data plus package_json when set."""
    data = list(upstream_data)
    if package_json == None:
        return data
    if type(package_json) != "string" and type(package_json) != "Label":
        fail("package_json must be a single label string, got %s: %r" % (type(package_json), package_json))
    key = label_key(package_json)
    for item in data:
        if label_key(item) == key:
            return data
    data.append(package_json)
    return data

def node_wrap_test(name, upstream_rule, forward_rule, srcs, upstream_data, package_json, upstream_attrs, env_inherit, visibility, kwargs):
    """Declares the manual upstream Node test and its forwarder."""
    effective_env = node_test_env(env_inherit)
    label_error = dx_args_label_error(
        name,
        kwargs.get("args"),
        list(srcs) + list(upstream_data),
        {"deps": kwargs.get("deps"), "tools": kwargs.get("tools")},
    )
    if label_error != None:
        fail(label_error)
    data = node_test_upstream_data(upstream_data, package_json, native.package_relative_label)
    upstream_kwargs = dx_test_upstream_kwargs(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    upstream_rule(
        name = name + "_upstream",
        data = data,
        env_inherit = effective_env,
        **(upstream_attrs | upstream_kwargs)
    )
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    if "args" in forward_kwargs:
        forward_kwargs["data"] = data
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        env_inherit = effective_env,
        visibility = visibility,
        **forward_kwargs
    )
