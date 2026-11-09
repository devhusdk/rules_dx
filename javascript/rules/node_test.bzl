"""Shared Jest and plain Node test core for the JavaScript and TypeScript wrappers."""

load("//libs/starlark:wrapper.bzl", "dx_args_label_error", "dx_forward_attrs", "dx_forward_data_attr", "dx_forwarded_quiet", "dx_lcov_merger_attr", "dx_quality_sources", "dx_run_env_attrs", "dx_symlink_default_info", "dx_test_forward_kwargs", "dx_run_environment", "dx_test_upstream_kwargs")
load("//quality:sources.bzl", "QualitySourcesInfo")

_JEST_ONLY_KEYS = [
    "config",
    "snapshots",
    "run_in_band",
    "colors",
    "auto_configure_reporters",
    "auto_configure_test_sequencer",
    "snapshots_ext",
    "quiet_snapshot_updates",
]

NODE_JEST_WINDOWS_ARGS = ["--coverageProvider=v8"]

def node_jest_upstream_attrs(node_modules, kwargs):
    """Returns the jest_test attributes dx sets; pops fixed_args from kwargs."""
    user_args = list(kwargs.pop("fixed_args", None) or [])
    return {
        "node_modules": node_modules,
        "fixed_args": user_args + select({
            Label("@platforms//os:windows"): NODE_JEST_WINDOWS_ARGS,
            "//conditions:default": [],
        }),
    }

def node_test_runner_rejection(rule, runner, node_modules, entry_point, kwargs):
    """Returns the rejection for a bad runner or runner arguments, or None."""
    if runner not in ("jest", "node"):
        return rule + ' runner must be "jest" or "node", got "' + str(runner) + '".'
    if runner == "jest":
        if node_modules == None:
            return rule + ' with runner = "jest" needs node_modules.'
        if entry_point != None:
            return rule + ' with runner = "jest" runs jest; set runner = "node" to use entry_point.'
        if kwargs.get("auto_configure_reporters", True) == False:
            return rule + ' with runner = "jest" keeps the standard reporters; set runner = "node" for a custom runner.'
        return None
    if entry_point == None:
        return rule + ' with runner = "node" needs entry_point.'
    if node_modules != None:
        return rule + ' with runner = "node" runs plain Node; `node_modules` needs runner = "jest".'
    for key in _JEST_ONLY_KEYS:
        if key in kwargs:
            return rule + ' with runner = "node" runs plain Node; `' + key + '` needs runner = "jest".'
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
            dx_run_environment(ctx, ctx.attr.upstream[RunEnvironmentInfo] if RunEnvironmentInfo in ctx.attr.upstream else None),
            dx_quality_sources(ctx.files.srcs, quality_specs, str(ctx.label)),
        ] + dx_forwarded_quiet(ctx.attr.upstream, [InstrumentedFilesInfo, OutputGroupInfo])

    return rule(
        implementation = _impl,
        test = True,
        provides = [DefaultInfo, RunEnvironmentInfo, QualitySourcesInfo],
        attrs = dx_forward_attrs(
            allow_files = allow_files,
            upstream_providers = [[DefaultInfo]],
            extra_attrs = dx_lcov_merger_attr() | dx_forward_data_attr() | dx_run_env_attrs(),
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

def node_test_upstream_kwargs(kwargs):
    """Returns the kwargs for the upstream test, without those it sets itself."""
    upstream_kwargs = dx_test_upstream_kwargs(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    upstream_kwargs.pop("testonly", None)
    return upstream_kwargs

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
    upstream_kwargs = node_test_upstream_kwargs(kwargs)
    upstream_rule(
        name = name + "_upstream",
        data = data,
        **(upstream_attrs | upstream_kwargs)
    )
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    forward_kwargs["env_inherit"] = effective_env
    if "args" in forward_kwargs or "env" in forward_kwargs:
        forward_kwargs["data"] = data
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **forward_kwargs
    )
