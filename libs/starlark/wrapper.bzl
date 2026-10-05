"""Shared wrapper-forwarder plumbing for language rules."""

load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

def dx_forwarded_runtime_providers(upstream, what, run_environment = None):
    """Forwards the upstream runtime providers every wrapper preserves."""
    out = []
    if InstrumentedFilesInfo not in upstream:
        fail(what + ": upstream target has no InstrumentedFilesInfo: " + str(upstream.label))
    out.append(upstream[InstrumentedFilesInfo])
    if OutputGroupInfo in upstream:
        out.append(upstream[OutputGroupInfo])
    if run_environment != None:
        out.append(run_environment)
    elif RunEnvironmentInfo in upstream:
        out.append(upstream[RunEnvironmentInfo])
    return out

def dx_missing_optional_names(requested_names, present_names):
    """Returns the requested names absent from the present names. See."""
    return [n for n in requested_names if n not in present_names]

def dx_optional_forward_warning(what, upstream_label, requested_names, missing_names):
    """Returns the skip warning for an optional forward, or None when nothing was skipped. See."""
    if len(missing_names) == 0:
        return None
    forwarded = len(requested_names) - len(missing_names)
    suffix = "; empty forward is expected when upstream omits the surface" if forwarded == 0 else ""
    return what + ": upstream " + upstream_label + " omits optional provider(s) " + ", ".join(missing_names) + " (forwarded " + str(forwarded) + " of " + str(len(requested_names)) + ")" + suffix

def dx_forwarded_optional(upstream, providers, what = "dx wrapper"):
    """Forwards the upstream providers that are present, warning on each skip."""
    missing = [p for p in providers if p not in upstream]
    if len(missing) > 0:
        warning = dx_optional_forward_warning(
            what,
            str(upstream.label),
            [str(p) for p in providers],
            [str(p) for p in missing],
        )
        if warning != None:
            print(warning)  # buildifier: disable=print  # intentional optional-provider skip warning, not debug
    return [upstream[p] for p in providers if p in upstream]

def dx_preserved_providers(upstream, required, what):
    """Returns the required upstream provider instances, failing when absent."""
    out = []
    for item in required:
        provider = item[0]
        name = item[1]
        if provider not in upstream:
            fail(what + ": upstream target has no " + name + ": " + str(upstream.label))
        out.append(upstream[provider])
    return out

DX_COMPATIBILITY_ATTRS = [
    "compatible_with",
    "exec_compatible_with",
    "target_compatible_with",
]

DX_LIBRARY_FORWARD_ATTRS = DX_COMPATIBILITY_ATTRS + [
    "aspect_hints",
    "hdrs",
    "tags",
    "testonly",
]

DX_BINARY_FORWARD_ATTRS = DX_COMPATIBILITY_ATTRS + [
    "aspect_hints",
    "tags",
    "testonly",
]

DX_TEST_ENV_ATTRS = [
    "env",
    "env_inherit",
]

DX_TEST_RUN_ATTRS = DX_TEST_ENV_ATTRS + [
    "flaky",
    "shard_count",
    "size",
    "tags",
    "timeout",
]

DX_TEST_FORWARD_ATTRS = DX_COMPATIBILITY_ATTRS + [
    "aspect_hints",
] + DX_TEST_RUN_ATTRS

def dx_forwarded_contract_kwargs(kwargs, names):
    """Selects the declared attributes one wrapper shape forwards."""
    out = {}
    for name in names:
        value = kwargs.get(name, None)
        if value != None:
            out[name] = value
    return out

def dx_merged_environment(upstream_environment, upstream_inherited, env, env_inherit):
    """Merges the upstream run environment with the declared fallback values."""
    environment = dict(env)
    environment.update(upstream_environment)
    inherited = list(upstream_inherited)
    for name in env_inherit:
        if name not in inherited:
            inherited.append(name)
    return environment, inherited

def dx_test_env_attrs():
    """Builds the declared environment attributes a test forwarder needs."""
    return {
        "env": attr.string_dict(),
        "env_inherit": attr.string_list(),
    }

def dx_forwarded_test_kwargs(kwargs):
    """Extracts the test attributes a public test forwarder declares."""
    return dx_forwarded_contract_kwargs(kwargs, DX_TEST_RUN_ATTRS)

def dx_quality_sources(files, specs, label):
    """Builds QualitySourcesInfo for direct wrapper sources."""
    buckets = {}
    order = []
    for spec in specs:
        key = spec[0]
        exts = spec[1] if type(spec[1]) == "list" else [spec[1]]
        excludes = spec[2] if len(spec) > 2 else []
        matched = [
            f
            for f in files
            if f.extension in exts and not _has_excluded_suffix(f.basename, excludes)
        ]
        if key not in buckets:
            buckets[key] = []
            order.append(key)
        buckets[key].extend(matched)
    direct_sources = {}
    for key in order:
        if len(buckets[key]) > 0:
            direct_sources[key] = depset(buckets[key])
    check_direct_sources(direct_sources, label)
    return QualitySourcesInfo(direct_sources = direct_sources)

def _has_excluded_suffix(basename, excludes):
    for suffix in excludes:
        if basename.endswith(suffix):
            return True
    return False

def dx_symlink_executable_name(name, upstream_executable):
    """Maps one forwarder output name to the upstream's own filename suffix."""
    basename = upstream_executable.basename
    dot = basename.rfind(".")
    if dot > 0:
        return name + basename[dot:]
    return name

def dx_symlink_executable(ctx, target_file):
    """Symlinks one upstream executable under the forwarder's own name."""
    link = ctx.actions.declare_file(
        dx_symlink_executable_name(ctx.label.name, target_file),
    )
    ctx.actions.symlink(output = link, target_file = target_file, is_executable = True)
    return link

def dx_symlink_default_info(ctx, what, extra_runfiles = None):
    """Builds the executable DefaultInfo symlinking the upstream binary."""
    upstream = ctx.attr.upstream[DefaultInfo]
    exe = upstream.files_to_run.executable
    if exe == None:
        fail(what + ": upstream target has no executable: " + str(ctx.attr.upstream.label))
    link = dx_symlink_executable(ctx, exe)
    runfiles = ctx.runfiles(files = [link]).merge(upstream.default_runfiles)
    if extra_runfiles != None:
        runfiles = runfiles.merge(extra_runfiles)
    return DefaultInfo(
        executable = link,
        files = depset([link]),
        runfiles = runfiles,
    )

def dx_lcov_merger_attr():
    """Returns the coverage _lcov_merger attribute for test forwarders."""
    return {
        "_lcov_merger": attr.label(
            default = configuration_field(fragment = "coverage", name = "output_generator"),
            executable = True,
            cfg = "exec",
        ),
    }

def dx_forward_attrs(allow_files, upstream_providers, extra_attrs = None):
    """Builds the common srcs/upstream attribute dict for forwarders."""
    upstream_attr_kwargs = {
        "mandatory": True,
    }
    if upstream_providers != None:
        upstream_attr_kwargs["providers"] = upstream_providers
    attrs = {
        "srcs": attr.label_list(
            allow_files = allow_files,
        ),
        "upstream": attr.label(**upstream_attr_kwargs),
    }
    if extra_attrs:
        attrs.update(extra_attrs)
    return attrs

def _dx_quality_files(ctx, extra_quality_attrs):
    files = list(ctx.files.srcs)
    for name in extra_quality_attrs or []:
        files.extend(getattr(ctx.files, name, []))
    return files

def _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs = None, run_environment = None):
    if runtime == "mandatory":
        return dx_forwarded_runtime_providers(upstream, what, run_environment)
    elif runtime == "besteffort":
        out = []
        if InstrumentedFilesInfo in upstream:
            out.append(upstream[InstrumentedFilesInfo])
        else:
            src_attrs = ["srcs"] + (list(extra_quality_attrs) if extra_quality_attrs else [])
            out.append(coverage_common.instrumented_files_info(
                ctx,
                source_attributes = src_attrs,
                dependency_attributes = ["upstream"],
            ))
        if OutputGroupInfo in upstream:
            out.append(upstream[OutputGroupInfo])
        if run_environment != None:
            out.append(run_environment)
        elif RunEnvironmentInfo in upstream:
            out.append(upstream[RunEnvironmentInfo])
        return out
    else:
        fail("dx wrapper: unknown runtime '" + runtime + "': want \"mandatory\" or \"besteffort\"")

def dx_merged_run_environment(upstream, env, env_inherit):
    """Returns the run environment merging the upstream values with the declared ones."""
    upstream_environment = {}
    upstream_inherited = []
    if RunEnvironmentInfo in upstream:
        upstream_environment = upstream[RunEnvironmentInfo].environment
        upstream_inherited = upstream[RunEnvironmentInfo].inherited_environment
    environment, inherited = dx_merged_environment(upstream_environment, upstream_inherited, env, env_inherit)
    return RunEnvironmentInfo(environment = environment, inherited_environment = inherited)

def dx_library_forward_rule(provides, required_providers, quality_specs, what, allow_files, upstream_providers, extra_attrs = None, runtime = "mandatory", extra_quality_attrs = None):
    """Creates the public forwarding rule for one library wrapper."""

    def _impl(ctx):
        upstream = ctx.attr.upstream
        return (
            dx_preserved_providers(upstream, required_providers, what) +
            [upstream[DefaultInfo]] +
            _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs) +
            [dx_quality_sources(_dx_quality_files(ctx, extra_quality_attrs), quality_specs, str(ctx.label))]
        )

    return rule(
        implementation = _impl,
        provides = provides,
        attrs = dx_forward_attrs(
            allow_files = allow_files,
            upstream_providers = upstream_providers,
            extra_attrs = extra_attrs,
        ),
    )

def dx_executable_forward_rule(kind, provides, required_providers, quality_specs, what, allow_files, upstream_providers, extra_attrs = None, optional_providers = [], runtime = "mandatory", extra_quality_attrs = None, coverage_runfiles = None, toolchains = None):
    """Creates the executable or test forwarding rule for one wrapper."""

    def _impl(ctx):
        upstream = ctx.attr.upstream
        extra_runfiles = coverage_runfiles(ctx) if coverage_runfiles != None else None
        run_environment = None
        if kind == "test":
            run_environment = dx_merged_run_environment(upstream, ctx.attr.env, ctx.attr.env_inherit)
        return (
            [dx_symlink_default_info(ctx, what, extra_runfiles)] +
            dx_preserved_providers(upstream, required_providers, what) +
            dx_forwarded_optional(upstream, optional_providers, what) +
            _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs, run_environment) +
            [dx_quality_sources(_dx_quality_files(ctx, extra_quality_attrs), quality_specs, str(ctx.label))]
        )

    attrs = dx_forward_attrs(
        allow_files = allow_files,
        upstream_providers = upstream_providers,
        extra_attrs = extra_attrs,
    )
    if kind == "test":
        attrs.update(dx_test_env_attrs())
    if kind == "executable":
        return rule(
            implementation = _impl,
            executable = True,
            provides = provides,
            attrs = attrs,
            toolchains = toolchains if toolchains != None else [],
        )
    elif kind == "test":
        return rule(
            implementation = _impl,
            test = True,
            provides = provides,
            attrs = attrs,
            toolchains = toolchains if toolchains != None else [],
        )
    else:
        fail("dx_executable_forward_rule: unknown kind '" + kind + "': want \"executable\" or \"test\"")

def dx_binary_forward_kwargs(kwargs):
    """Returns the forwarder kwargs for one binary shape."""
    return dx_forwarded_contract_kwargs(kwargs, DX_BINARY_FORWARD_ATTRS)

def dx_test_upstream_kwargs(kwargs, srcs = None):
    """Returns the private upstream kwargs for one test shape."""
    out = dict(kwargs)
    if "tags" in out:
        kept = [t for t in out["tags"] if t != "manual"]
        if len(kept) > 0:
            out["tags"] = kept
        else:
            out.pop("tags")
    out["visibility"] = ["//visibility:private"]
    if srcs != None:
        out["srcs"] = srcs
    return out

def dx_test_forward_kwargs(kwargs):
    """Returns the forwarder kwargs for one test shape."""
    return dx_forwarded_contract_kwargs(kwargs, DX_TEST_FORWARD_ATTRS)

def _dx_forward_only(declared, upstream_kwargs, names):
    """Removes the forwarder-only attributes from the upstream kwargs."""
    forward_only = {}
    for name in names:
        value = declared.get(name, None)
        upstream_kwargs.pop(name, None)
        if value != None:
            forward_only[name] = value
    return forward_only

def dx_wrap_binary(name, upstream_rule, forward_rule, srcs, visibility = None, upstream_kwargs = None, **kwargs):
    """Instantiates one private upstream binary plus its public forwarder."""
    effective = dict(upstream_kwargs) if upstream_kwargs != None else dict(kwargs)
    if len(srcs) > 0:
        effective["srcs"] = srcs
    elif "srcs" in effective:
        effective.pop("srcs")
    effective["visibility"] = ["//visibility:private"]
    upstream_rule(
        name = name + "_upstream",
        **effective
    )
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **dx_binary_forward_kwargs(kwargs)
    )

def dx_wrap_test(name, upstream_rule, forward_rule, srcs, visibility = None, upstream_kwargs = None, extra_forward_kwargs = None, forward_only_attrs = [], **kwargs):
    """Instantiates one private upstream test plus its public forwarder."""
    base = dict(upstream_kwargs) if upstream_kwargs != None else dict(kwargs)
    forward_only = _dx_forward_only(kwargs, base, forward_only_attrs)
    effective = dx_test_upstream_kwargs(base, srcs = srcs)
    forward_srcs = srcs if srcs != None else []
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    forward_kwargs.update(forward_only)
    if extra_forward_kwargs:
        forward_kwargs.update(extra_forward_kwargs)
    upstream_rule(
        name = name + "_upstream",
        **effective
    )
    forward_rule(
        name = name,
        testonly = True,
        upstream = name + "_upstream",
        srcs = forward_srcs,
        visibility = visibility,
        **forward_kwargs
    )

def dx_framework_forward_rule(language, ext):
    """Creates the forwarding rule for one JS-framework wrapper."""
    return dx_library_forward_rule(
        provides = [JsInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
        required_providers = [(JsInfo, "JsInfo")],
        quality_specs = [(language, language)],
        what = language + "_*",
        allow_files = [ext],
        upstream_providers = [[JsInfo]],
    )

def dx_framework_library(name, srcs, upstream_rule, forward_rule, visibility = None, **kwargs):
    """Instantiates one JS-framework library with standard skip tags."""
    tags = list(kwargs.pop("tags", []))
    for tag in ["no-format", "no-lint", "no-typecheck"]:
        if tag not in tags:
            tags.append(tag)
    kwargs["tags"] = tags
    dx_wrap(name, upstream_rule, forward_rule, srcs, visibility = visibility, **kwargs)

def dx_wrap(name, upstream_rule, forward_rule, srcs, visibility = None, forward_only_attrs = [], **kwargs):
    """Instantiates one private upstream target plus its public forwarder."""
    upstream_kwargs = dict(kwargs)
    forward_only = _dx_forward_only(kwargs, upstream_kwargs, forward_only_attrs)
    upstream_kwargs.pop("tags", None)
    upstream_rule(
        name = name + "_upstream",
        srcs = srcs,
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward_kwargs = dx_forwarded_contract_kwargs(kwargs, DX_LIBRARY_FORWARD_ATTRS)
    forward_kwargs.update(forward_only)
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **forward_kwargs
    )
