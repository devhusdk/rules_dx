"""Shared wrapper-forwarder plumbing for language rules."""

load("//libs/starlark:resources.bzl", "dx_test_resource_tags")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo", "check_direct_sources")

DX_EXEC_PROVIDES = [DefaultInfo, QualitySourcesInfo]

def dx_forwarded_runtime_providers(upstream, what):
    """Forwards the upstream runtime providers every wrapper preserves."""
    out = []
    if InstrumentedFilesInfo not in upstream:
        fail(what + ": upstream target has no InstrumentedFilesInfo: " + str(upstream.label))
    out.append(upstream[InstrumentedFilesInfo])
    if OutputGroupInfo in upstream:
        out.append(upstream[OutputGroupInfo])
    if RunEnvironmentInfo in upstream:
        out.append(upstream[RunEnvironmentInfo])
    return out

def dx_missing_optional_names(requested_names, present_names):
    """Returns the requested names absent from the present names."""
    return [n for n in requested_names if n not in present_names]

def dx_optional_forward_warning(what, upstream_label, requested_names, missing_names):
    """Returns the skip warning for an optional forward, or None when nothing was skipped."""
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
            print(warning)  # buildifier: disable=print
    return [upstream[p] for p in providers if p in upstream]

def dx_forwarded_quiet(upstream, providers):
    """Forwards the upstream providers that are present without warning on absence."""
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

def dx_effective_visibility(visibility):
    """Returns the explicit forwarder visibility for a public export."""
    if visibility == None:
        return ["//visibility:private"]
    return visibility

_DX_TEST_FORWARDER_EXEC_GROUPS = ("test",)

def _dx_test_exec_group_keys(groups):
    """Keeps only the exec group keys a test forwarder declares."""
    kept = {key: groups[key] for key in _DX_TEST_FORWARDER_EXEC_GROUPS if key in groups}
    return kept if kept else None

def dx_forwarded_test_kwargs(kwargs):
    """Extracts the standard test attributes a test forwarder preserves."""
    out = {}
    if "tags" in kwargs and kwargs["tags"] != None:
        out["tags"] = list(kwargs["tags"])
    for key in ("timeout", "shard_count", "size", "local", "exec_compatible_with", "exec_properties", "flaky", "args"):
        if key in kwargs and kwargs[key] != None:
            out[key] = kwargs[key]
    if "args" in out and kwargs.get("data", None) != None:
        out["data"] = kwargs["data"]
    if kwargs.get("exec_group_compatible_with", None) != None:
        groups = _dx_test_exec_group_keys(kwargs["exec_group_compatible_with"])
        if groups != None:
            out["exec_group_compatible_with"] = groups
    return out

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
    prefix = name + "_upstream"
    if basename.startswith(prefix):
        rest = basename[len(prefix):]
        if rest == "" or rest.startswith("."):
            return name + rest
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

def dx_upstream(ctx):
    """Returns the upstream target, unwrapping a transitioned attribute."""
    upstream = ctx.attr.upstream
    return upstream[0] if type(upstream) == "list" else upstream

def dx_symlink_default_info(ctx, what, extra_runfiles = None):
    """Builds the executable DefaultInfo symlinking the upstream binary."""
    upstream = dx_upstream(ctx)[DefaultInfo]
    exe = upstream.files_to_run.executable
    if exe == None:
        fail(what + ": upstream target has no executable: " + str(dx_upstream(ctx).label))
    link = dx_symlink_executable(ctx, exe)
    runfiles = ctx.runfiles(files = [link]).merge(upstream.default_runfiles)
    for dep in getattr(ctx.attr, "data", []):
        runfiles = runfiles.merge(dep[DefaultInfo].default_runfiles)
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

def dx_collect_cc_coverage_attr(collector):
    """Returns the C/C++ coverage collector attribute for test forwarders."""
    return {
        "_collect_cc_coverage": attr.label(
            default = collector,
            executable = True,
            cfg = config.exec(exec_group = "test"),
        ),
    }

DxDataCoverageInfo = provider(
    doc = "Coverage files of the data deps of one test.",
    fields = {
        "instrumented_files": "Depset of instrumented source files.",
        "metadata_files": "Depset of coverage metadata files.",
    },
)

def dx_native_coverage_attrs():
    """Returns the attributes native (C/C++/LLVM) coverage collection needs."""
    return dx_collect_cc_coverage_attr(Label("//tools/coverage/collect:collect_native")) | {
        "_llvm_cov": attr.label(
            default = Label("@llvm//tools:llvm-cov"),
            executable = True,
            allow_files = True,
            cfg = config.exec(exec_group = "test"),
        ),
        "_llvm_profdata": attr.label(
            default = Label("@llvm//tools:llvm-profdata"),
            executable = True,
            allow_files = True,
            cfg = config.exec(exec_group = "test"),
        ),
    }

def dx_native_test_coverage(ctx, upstream):
    """Returns coverage providers that collect native LLVM coverage."""
    collector = ctx.executable._collect_cc_coverage
    llvm_cov = ctx.executable._llvm_cov
    llvm_profdata = ctx.executable._llvm_profdata
    metadata = [collector, llvm_cov, llvm_profdata]
    transitive = []
    if DxDataCoverageInfo in upstream:
        data = upstream[DxDataCoverageInfo]
        transitive = [data.metadata_files, data.instrumented_files]
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        dependency_attributes = ["upstream"],
        metadata_files = depset(metadata, transitive = transitive).to_list(),
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    if DxDataCoverageInfo not in upstream:
        env["DX_COVERAGE_CLANG_OBJECTS"] = "test_binary"
    env["CC_CODE_COVERAGE_SCRIPT"] = collector.path
    env["LLVM_COV"] = llvm_cov.path
    env["LLVM_PROFDATA"] = llvm_profdata.path
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = None,
    )

def _dx_data_coverage_aspect_impl(_target, ctx):
    infos = [dep[InstrumentedFilesInfo] for dep in getattr(ctx.rule.attr, "data", []) if InstrumentedFilesInfo in dep]
    return [DxDataCoverageInfo(
        instrumented_files = depset(transitive = [info.instrumented_files for info in infos]),
        metadata_files = depset(transitive = [info.metadata_files for info in infos]),
    )]

dx_data_coverage_aspect = aspect(
    implementation = _dx_data_coverage_aspect_impl,
    doc = "Collects coverage files of a test's data deps.",
)

def dx_forward_data_attr():
    """Returns the data attribute forwarders use to expand args."""
    return {"data": attr.label_list(allow_files = True)}

def dx_forward_attrs(allow_files, upstream_providers, extra_attrs = None, upstream_cfg = None, upstream_aspects = None):
    """Builds the common srcs/upstream attribute dict for forwarders."""
    upstream_attr_kwargs = {
        "mandatory": True,
    }
    if upstream_aspects != None:
        upstream_attr_kwargs["aspects"] = upstream_aspects
    if upstream_cfg != None:
        upstream_attr_kwargs["cfg"] = upstream_cfg
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

def _dx_tool_attrs(quality_tools):
    attrs = {}
    for tool_id, label in (quality_tools or {}).items():
        attrs["_dx_tool_" + tool_id] = attr.label(default = label, cfg = "exec", executable = True)
    return attrs

def _dx_tools_providers(ctx, quality_tools):
    if not quality_tools:
        return []
    tools = {}
    for tool_id in quality_tools:
        tools[tool_id] = getattr(ctx.attr, "_dx_tool_" + tool_id)[DefaultInfo].files_to_run
    return [QualityToolsInfo(tools = tools)]

def _dx_quality_files(ctx, extra_quality_attrs):
    files = list(ctx.files.srcs)
    for name in extra_quality_attrs or []:
        files.extend(getattr(ctx.files, name, []))
    return files

def _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs = None):
    if runtime == "mandatory":
        return dx_forwarded_runtime_providers(upstream, what)
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
        if RunEnvironmentInfo in upstream:
            out.append(upstream[RunEnvironmentInfo])
        return out
    else:
        fail("dx wrapper: unknown runtime '" + runtime + "': want \"mandatory\" or \"besteffort\"")

def dx_library_forward_rule(provides, required_providers, quality_specs, what, allow_files, upstream_providers, extra_attrs = None, runtime = "mandatory", extra_quality_attrs = None, quiet_providers = [], upstream_cfg = None, quality_tools = None):
    """Creates the public forwarding rule for one library wrapper."""

    def _impl(ctx):
        upstream = dx_upstream(ctx)
        return (
            dx_preserved_providers(upstream, required_providers, what) +
            dx_forwarded_quiet(upstream, quiet_providers) +
            [upstream[DefaultInfo]] +
            _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs) +
            [dx_quality_sources(_dx_quality_files(ctx, extra_quality_attrs), quality_specs, str(ctx.label))] +
            _dx_tools_providers(ctx, quality_tools)
        )

    attrs = dx_forward_attrs(
        allow_files = allow_files,
        upstream_providers = upstream_providers,
        extra_attrs = extra_attrs,
        upstream_cfg = upstream_cfg,
    )
    attrs.update(_dx_tool_attrs(quality_tools))
    return rule(
        implementation = _impl,
        provides = provides,
        attrs = attrs,
    )

def _dx_coverage_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs, coverage):
    if coverage == None:
        return _dx_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs)
    out = [coverage.instrumented, coverage.env]
    if OutputGroupInfo in upstream:
        out.append(upstream[OutputGroupInfo])
    return out

def dx_executable_forward_rule(kind, provides, required_providers, quality_specs, what, allow_files, upstream_providers, extra_attrs = None, optional_providers = [], runtime = "mandatory", extra_quality_attrs = None, coverage_runfiles = None, toolchains = None, quiet_providers = [], upstream_cfg = None, upstream_aspects = None, coverage_providers = None, quality_tools = None):
    """Creates the executable or test forwarding rule for one wrapper."""

    def _impl(ctx):
        upstream = dx_upstream(ctx)
        extra_runfiles = coverage_runfiles(ctx) if coverage_runfiles != None else None
        coverage = None
        if coverage_providers != None and ctx.configuration.coverage_enabled:
            coverage = coverage_providers(ctx, upstream)
        if coverage != None and coverage.runfiles != None:
            extra_runfiles = coverage.runfiles if extra_runfiles == None else extra_runfiles.merge(coverage.runfiles)
        return (
            [dx_symlink_default_info(ctx, what, extra_runfiles)] +
            dx_preserved_providers(upstream, required_providers, what) +
            dx_forwarded_optional(upstream, optional_providers, what) +
            dx_forwarded_quiet(upstream, quiet_providers) +
            _dx_coverage_runtime_providers(ctx, upstream, what, runtime, extra_quality_attrs, coverage) +
            [dx_quality_sources(_dx_quality_files(ctx, extra_quality_attrs), quality_specs, str(ctx.label))] +
            _dx_tools_providers(ctx, quality_tools)
        )

    attrs = dx_forward_attrs(
        allow_files = allow_files,
        upstream_providers = upstream_providers,
        extra_attrs = extra_attrs,
        upstream_cfg = upstream_cfg,
        upstream_aspects = upstream_aspects,
    )
    attrs.update(_dx_tool_attrs(quality_tools))
    if kind == "test" or kind == "executable":
        attrs.update(dx_forward_data_attr())
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

def dx_append_flags(kwargs, key, flags):
    """Returns a copy of kwargs with flags appended to the list or select() at key."""
    out = dict(kwargs)
    value = out.get(key, None)
    if value == None:
        out[key] = list(flags)
    elif type(value) == "select":
        out[key] = value + flags
    else:
        merged = list(value)
        for flag in flags:
            if flag not in merged:
                merged.append(flag)
        out[key] = merged
    return out

def dx_setdefaults(kwargs, defaults):
    """Returns a copy of kwargs with each absent default key set."""
    out = dict(kwargs)
    for key, value in defaults.items():
        out.setdefault(key, value)
    return out

def dx_wrap_forward_kwargs(kwargs):
    """Returns the forwarder kwargs for one library shape."""
    out = {}
    for key in ("aspect_hints", "tags", "testonly", "target_compatible_with"):
        if kwargs.get(key, None) != None:
            out[key] = kwargs[key]
    return out

def dx_binary_run_kwargs(kwargs):
    """Returns args, plus data only when args is set, for a binary forwarder."""
    out = {}
    if kwargs.get("args", None) != None:
        out["args"] = kwargs["args"]
        if kwargs.get("data", None) != None:
            out["data"] = kwargs["data"]
    return out

def dx_binary_forward_kwargs(kwargs):
    """Returns the forwarder kwargs for one binary shape."""
    out = dx_binary_run_kwargs(kwargs)
    if kwargs.get("testonly", None) != None:
        out["testonly"] = kwargs["testonly"]
    if kwargs.get("tags", None) != None:
        out["tags"] = kwargs["tags"]
    if kwargs.get("aspect_hints", None) != None:
        out["aspect_hints"] = kwargs["aspect_hints"]
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

def dx_upstream_tags(tags):
    """Returns private upstream tags: user tags with skip markers, plus manual."""
    out = upstream_tags(tags)
    if "manual" not in out:
        out = out + ["manual"]
    return out

def dx_src_path(package, src):
    """Returns the repo-relative path of a string src, or None."""
    if type(src) != "string" or src.startswith("@"):
        return None
    if src.startswith("//"):
        pkg, sep, target = src[2:].partition(":")
        if not sep:
            target = pkg.split("/")[-1]
        return pkg + "/" + target if pkg else target
    if src.startswith(":"):
        src = src[1:]
    return package + "/" + src if package else src

def dx_test_upstream_kwargs(kwargs, srcs = None):
    """Returns the private upstream kwargs for one test shape."""
    out = dict(kwargs)
    out["tags"] = dx_upstream_tags(out.get("tags", None))
    out["visibility"] = ["//visibility:private"]
    if srcs != None:
        out["srcs"] = srcs
    return out

def dx_binary_upstream_kwargs(kwargs, srcs):
    """Returns the private upstream kwargs for one binary shape."""
    out = dict(kwargs)
    if srcs:
        out["srcs"] = srcs
    elif "srcs" in out:
        out.pop("srcs")
    out["tags"] = upstream_tags(kwargs.get("tags", None))
    out["visibility"] = ["//visibility:private"]
    return out

def dx_wrap_upstream_kwargs(kwargs, srcs):
    """Returns the private upstream kwargs for one library shape."""
    out = {key: value for key, value in kwargs.items() if key != "tags"}
    out["srcs"] = srcs
    out["tags"] = dx_upstream_tags(kwargs.get("tags", None))
    out["visibility"] = ["//visibility:private"]
    return out

def dx_test_forward_kwargs(kwargs):
    """Returns the forwarder kwargs for one test shape."""
    out = dx_forwarded_test_kwargs(kwargs)
    tags = dx_test_resource_tags(kwargs.get("size", None), kwargs.get("tags", None))
    if tags or "tags" in out:
        out["tags"] = tags
    if kwargs.get("aspect_hints", None) != None:
        out["aspect_hints"] = kwargs["aspect_hints"]
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

def dx_wrap_binary(name, upstream_rule, forward_rule, srcs, visibility = None, upstream_kwargs = None, extra_forward_kwargs = None, quality_srcs = None, **kwargs):
    """Instantiates one private upstream binary plus its public forwarder."""
    effective = dx_binary_upstream_kwargs(upstream_kwargs if upstream_kwargs != None else kwargs, srcs)
    upstream_rule(
        name = name + "_upstream",
        **effective
    )
    forward_kwargs = dx_binary_forward_kwargs(kwargs)
    if extra_forward_kwargs:
        forward_kwargs.update(extra_forward_kwargs)
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = quality_srcs if quality_srcs != None else (srcs or []),
        visibility = visibility,
        **forward_kwargs
    )

def dx_wrap_test(name, upstream_rule, forward_rule, srcs, visibility = None, upstream_kwargs = None, extra_forward_kwargs = None, quality_srcs = None, **kwargs):
    """Instantiates one private upstream test plus its public forwarder."""
    base = dict(upstream_kwargs) if upstream_kwargs != None else dict(kwargs)
    effective = dx_test_upstream_kwargs(base, srcs = srcs)
    forward_srcs = quality_srcs if quality_srcs != None else (srcs if srcs != None else [])
    forward_kwargs = dx_test_forward_kwargs(kwargs)
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

def dx_wrap(name, upstream_rule, forward_rule, srcs, visibility = None, upstream_kwargs = None, extra_forward_kwargs = None, quality_srcs = None, executable = False, **kwargs):
    """Instantiates one private upstream target plus its public forwarder."""
    upstream_rule(
        name = name + "_upstream",
        **dx_wrap_upstream_kwargs(upstream_kwargs if upstream_kwargs != None else kwargs, srcs)
    )
    forward_kwargs = dx_wrap_forward_kwargs(kwargs)
    if executable:
        forward_kwargs.update(dx_binary_run_kwargs(kwargs))
    if extra_forward_kwargs:
        forward_kwargs.update(extra_forward_kwargs)
    forward_rule(
        name = name,
        upstream = name + "_upstream",
        srcs = quality_srcs if quality_srcs != None else srcs,
        visibility = visibility,
        **forward_kwargs
    )

def dx_forwarded_run_environment(upstream, environment = {}, inherited = []):
    """Returns RunEnvironmentInfo merging the upstream's environment with extras.

    Args:
        upstream: The upstream target; its RunEnvironmentInfo, if any, is merged first.
        environment: Extra variables; they override upstream values.
        inherited: Extra inherited variable names, unioned with the upstream's.

    Returns:
        A RunEnvironmentInfo.
    """
    env = {}
    names = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        names.extend(upstream[RunEnvironmentInfo].inherited_environment)
    env.update(environment)
    for item in inherited:
        if item not in names:
            names.append(item)
    return RunEnvironmentInfo(environment = env, inherited_environment = names)

_ARGS_LOCATION_FUNCTIONS = [
    "location",
    "locations",
    "rootpath",
    "rootpaths",
    "execpath",
    "execpaths",
    "rlocationpath",
    "rlocationpaths",
]

def dx_args_label_references(args):
    """Returns the labels that $(location)-style expansions in args name."""
    labels = []
    for arg in args or []:
        for part in arg.split("$(")[1:]:
            if ")" not in part:
                continue
            tokens = part.split(")")[0].split(" ")
            tokens = [t for t in tokens if t]
            if len(tokens) < 2 or tokens[0] not in _ARGS_LOCATION_FUNCTIONS:
                continue
            for label in tokens[1:]:
                if label.startswith(":") or label.startswith("//") or label.startswith("@"):
                    labels.append(label)
    return labels

def dx_args_label_error(name, args, visible_labels, upstream_only):
    """Returns an error when args expand a label the forwarder cannot see.

    The forwarder only sees `data` and `srcs`; labels that appear only in other
    upstream attributes (for example deps or tools) do not resolve there.

    Args:
        name: The target name, for the message.
        args: The test args.
        visible_labels: Labels in the forwarder's data and srcs.
        upstream_only: Dict from attribute name (deps, tools) to its labels.

    Returns:
        The error string, or None when every referenced label is visible.
    """
    visible = [str(native.package_relative_label(l)) for l in visible_labels]
    for label in dx_args_label_references(args):
        key = str(native.package_relative_label(label))
        if key in visible:
            continue
        for attr_name, labels in upstream_only.items():
            if key in [str(native.package_relative_label(l)) for l in labels or []]:
                return "%s: args names %s, which is only in %s; add it to data" % (name, label, attr_name)
    return None
