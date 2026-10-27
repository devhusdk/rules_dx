"""Shared wrapper conformance checks for language rules."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "expect_equal", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_effective_visibility", "dx_forwarded_test_kwargs", "dx_symlink_executable_name")
load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES", "QualitySourcesInfo")

def dx_file_class_checks(classes):
    """Builds known-class checks for each file class."""
    checks = []
    for file_class in classes:
        checks.append(expect_equal(file_class + " class is known", file_class in KNOWN_SEMANTIC_FILE_CLASSES, True))
    return checks

def dx_forwarder_checks():
    """Builds shared visibility and test kwarg checks."""
    return [
        expect_equal("forwarder defaults to private", dx_effective_visibility(None), ["//visibility:private"]),
        expect_equal("explicit visibility wins", dx_effective_visibility(["//visibility:public"]), ["//visibility:public"]),
        expect_equal("test kwargs keep manual", dx_forwarded_test_kwargs({"tags": ["manual", "cpu:4"]}), {"tags": ["manual", "cpu:4"]}),
        expect_equal("test kwargs forward timeout and flaky", dx_forwarded_test_kwargs({"timeout": "short", "flaky": True}), {"timeout": "short", "flaky": True}),
        expect_equal("test kwargs forward flaky", dx_forwarded_test_kwargs({"flaky": True}), {"flaky": True}),
        expect_equal("test kwargs forward args", dx_forwarded_test_kwargs({"args": ["--x"]}), {"args": ["--x"]}),
        expect_equal("test kwargs forward data with args", dx_forwarded_test_kwargs({"args": ["--x"], "data": [":d"]}), {"args": ["--x"], "data": [":d"]}),
        expect_equal("test kwargs keep data upstream without args", dx_forwarded_test_kwargs({"data": [":d"]}), {}),
        expect_equal("test kwargs keep env upstream", dx_forwarded_test_kwargs({"env": {"K": "1"}}), {}),
        expect_equal("test kwargs keep env_inherit upstream", dx_forwarded_test_kwargs({"env_inherit": ["HOME"]}), {}),
        expect_equal("test kwargs forward exec_properties", dx_forwarded_test_kwargs({"exec_properties": {"Pool": "gpu"}}), {"exec_properties": {"Pool": "gpu"}}),
        expect_equal("test kwargs drop other exec group properties", dx_forwarded_test_kwargs({"exec_properties": {"cpp_link.Pool": "c"}}), {}),
        expect_equal("test kwargs keep test and unprefixed properties", dx_forwarded_test_kwargs({"exec_properties": {"Pool": "a", "test.Pool": "b", "cpp_link.Pool": "c"}}), {"exec_properties": {"Pool": "a", "test.Pool": "b"}}),
        expect_equal("test kwargs forward exec_compatible_with", dx_forwarded_test_kwargs({"exec_compatible_with": ["@platforms//os:linux"]}), {"exec_compatible_with": ["@platforms//os:linux"]}),
        expect_equal("test kwargs forward test exec group", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}), {"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}),
        expect_equal("test kwargs keep other exec groups upstream", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"cpp_link": ["@platforms//os:linux"]}}), {}),
        expect_equal("test kwargs forward only test exec group", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}}), {"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}),
        expect_equal("test kwargs forward local", dx_forwarded_test_kwargs({"local": True}), {"local": True}),
        expect_equal("test kwargs keep local False", dx_forwarded_test_kwargs({"local": False}), {"local": False}),
        expect_equal("binary kwargs forward testonly", dx_binary_forward_kwargs({"testonly": True}), {"testonly": True}),
        expect_equal("binary kwargs forward args with data", dx_binary_forward_kwargs({"args": ["--x"], "data": [":d"]}), {"args": ["--x"], "data": [":d"]}),
        expect_equal("binary kwargs keep data upstream without args", dx_binary_forward_kwargs({"data": [":d"]}), {}),
        expect_equal("binary kwargs drop none args", dx_binary_forward_kwargs({"args": None}), {}),
        expect_equal("test kwargs empty stays empty", dx_forwarded_test_kwargs({}), {}),
    ]

DX_COVERAGE_ENV = ["CC_CODE_COVERAGE_SCRIPT", "DX_COVERAGE_CLANG_OBJECTS", "LLVM_COV", "LLVM_PROFDATA"]

def dx_render_run_env(info):
    """Renders a RunEnvironmentInfo env as sorted K=V text, without the dx coverage collector env."""
    if info == None:
        return "(none)"
    return ",".join([k + "=" + info.environment[k] for k in sorted(info.environment.keys()) if k not in DX_COVERAGE_ENV])

def dx_render_run_inherited(info):
    """Renders a RunEnvironmentInfo inherited env as sorted text."""
    if info == None:
        return "(none)"
    return ",".join(sorted(info.inherited_environment))

_UNSET = "(unset)"

_DxWrapperAttrsInfo = provider(
    doc = "Attribute and provider facts about one wrapper and its private upstream.",
    fields = {
        "upstream": "Facts about the upstream target, or None when the target has no upstream attribute.",
        "wrapper": "Facts about this target.",
    },
)

def _attr_text(rule_attr, name):
    """Renders one rule attribute value, or (unset) when the rule has none."""
    value = getattr(rule_attr, name, _UNSET)
    if value == _UNSET or value == None:
        return _UNSET
    if type(value) == "list":
        return ",".join([strip_canonical(str(v.label)) if type(v) == "Target" else str(v) for v in value])
    return str(value)

def _target_facts(target, rule_attr):
    """Collects the comparable attribute and provider facts of one target."""
    info = target[DefaultInfo]
    exe = info.files_to_run.executable if info.files_to_run != None else None
    run_env = target[RunEnvironmentInfo] if RunEnvironmentInfo in target else None
    return struct(
        args = _attr_text(rule_attr, "args"),
        aspect_hints = _attr_text(rule_attr, "aspect_hints"),
        env = dx_render_run_env(run_env),
        executable = exe,
        inherited = dx_render_run_inherited(run_env),
        instrumented = InstrumentedFilesInfo in target,
        quality = target[QualitySourcesInfo] if QualitySourcesInfo in target else None,
        runfiles = info.default_runfiles,
        testonly = _attr_text(rule_attr, "testonly"),
    )

def _dx_wrapper_attrs_aspect_impl(target, ctx):
    """Records wrapper attributes and, through the upstream attribute, the private upstream's."""
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "list":
        upstream = upstream[0] if len(upstream) > 0 else None
    upstream_facts = None
    if upstream != None and _DxWrapperAttrsInfo in upstream:
        upstream_facts = upstream[_DxWrapperAttrsInfo].wrapper
    return [_DxWrapperAttrsInfo(
        upstream = upstream_facts,
        wrapper = _target_facts(target, ctx.rule.attr),
    )]

_dx_wrapper_attrs_aspect = aspect(
    implementation = _dx_wrapper_attrs_aspect_impl,
    doc = "Records the forwarded attributes of a wrapper and its upstream.",
    attr_aspects = ["upstream"],
)

def _strip_exe(name):
    return name[:-4] if name.endswith(".exe") else name

def _runfiles_paths(runfiles):
    if runfiles == None:
        return {}
    return {f.short_path: True for f in runfiles.files.to_list()}

def _dx_wrapper_forward_subject_impl(ctx):
    """Observes whether one wrapper forwards what its upstream provides."""
    attrs = ctx.attr.wrapper[_DxWrapperAttrsInfo]
    wrapper = attrs.wrapper
    upstream = attrs.upstream
    if upstream == None:
        fail("dx_wrapper_forward_subject: " + str(ctx.attr.wrapper.label) + " has no upstream attribute")
    wrapper_quality = wrapper.quality != None and len(wrapper.quality.direct_sources) > 0
    fields = {
        "args_match": str(wrapper.args == upstream.args),
        "aspect_hints_match": str(wrapper.aspect_hints == upstream.aspect_hints),
        "env_match": str(wrapper.env == upstream.env and wrapper.inherited == upstream.inherited),
        "kind": ctx.attr.kind,
        "quality_only_on_wrapper": str(wrapper.quality != None and upstream.quality == None),
        "testonly_match": str(wrapper.testonly == upstream.testonly),
        "upstream_instrumented": str(upstream.instrumented),
        "wrapper_instrumented": str(wrapper.instrumented),
        "wrapper_quality_sources": str(wrapper_quality),
    }
    if ctx.attr.kind in _EXECUTABLE_KINDS:
        executable_forwarded = False
        if wrapper.executable != None and upstream.executable != None:
            want = dx_symlink_executable_name(ctx.attr.wrapper.label.name, upstream.executable)
            executable_forwarded = _strip_exe(wrapper.executable.basename) == _strip_exe(want)
        fields["executable_forwarded"] = str(executable_forwarded)
        wrapper_paths = _runfiles_paths(wrapper.runfiles)
        upstream_paths = _runfiles_paths(upstream.runfiles)
        fields["runfiles_forwarded"] = str(len(upstream_paths) > 0 and all([p in wrapper_paths for p in upstream_paths]))
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

_LIBRARY_KIND = "library"

_EXECUTABLE_KINDS = ["binary", "test"]

DX_WRAPPER_FORWARD_KINDS = [_LIBRARY_KIND] + _EXECUTABLE_KINDS

dx_wrapper_forward_subject = rule(
    implementation = _dx_wrapper_forward_subject_impl,
    doc = "Compares one wrapper against its private upstream for forwarded providers and attributes.",
    attrs = {
        "kind": attr.string(
            doc = "Kind of the wrapped target, one of library, binary, or test.",
            mandatory = True,
            values = DX_WRAPPER_FORWARD_KINDS,
        ),
        "wrapper": attr.label(
            doc = "Wrapper target compared against its private upstream.",
            mandatory = True,
            aspects = [_dx_wrapper_attrs_aspect],
        ),
    },
)

def _forward_fields(kind):
    """Returns the expected all-forwarded fields for one wrapper kind."""
    fields = {
        "args_match": "True",
        "aspect_hints_match": "True",
        "env_match": "True",
        "kind": kind,
        "quality_only_on_wrapper": "True",
        "testonly_match": "True",
        "upstream_instrumented": "True",
        "wrapper_instrumented": "True",
        "wrapper_quality_sources": "True",
    }
    if kind in _EXECUTABLE_KINDS:
        fields["executable_forwarded"] = "True"
        fields["runfiles_forwarded"] = "True"
    return fields

def dx_wrapper_forward_observations(package, name, kinds, overrides = None):
    """Renders the analysis observations of the forward subjects dx_wrapper_contract_tests creates."""
    extra = overrides if overrides != None else {}
    lines = []
    subjects = sorted([("//" + package + ":" + name + "_" + kind, kind) for kind in kinds])
    for label, kind in subjects:
        fields = _forward_fields(kind)
        fields.update(extra.get(kind, {}))
        lines.append("subject " + label)
        lines.append("file " + label.split(":")[-1] + ".txt")
        for key in sorted(fields.keys()):
            lines.append("field " + key + "=" + fields[key])
        lines.append("aspect_field aspect_seen=True")
        lines.append("aspect_field field_count=" + str(len(fields)))
        lines.append("aspect_field has_subject=True")
        lines.append("aspect_field subject_label=" + label)
        lines.append("aspect_field transitive_count=0")
    return "\n".join(lines)

def dx_wrapper_contract_tests(name, file_classes = None, extra_checks = None, include_forwarder_checks = True, wrappers = None, known_gaps = None):
    """Instantiates one wrapper contract test from shared checks."""
    classes = file_classes if file_classes != None else []
    extras = extra_checks if extra_checks != None else []
    checks = []
    checks.extend(dx_file_class_checks(classes))
    if include_forwarder_checks:
        checks.extend(dx_forwarder_checks())
    checks.extend(extras)
    starlark_test(
        name = name,
        mode = "unit",
        checks = checks,
    )
    if wrappers == None:
        return
    kinds = sorted(wrappers.keys())
    for kind in kinds:
        if kind not in DX_WRAPPER_FORWARD_KINDS:
            fail("dx_wrapper_contract_tests: unknown wrapper kind '" + kind + "': want one of " + ", ".join(DX_WRAPPER_FORWARD_KINDS))
        dx_wrapper_forward_subject(
            name = name + "_" + kind,
            testonly = True,
            kind = kind,
            wrapper = wrappers[kind],
        )
    starlark_test(
        name = name + "_forward",
        mode = "analysis",
        testonly = True,
        subjects = [":" + name + "_" + kind for kind in kinds],
        expected_observations = dx_wrapper_forward_observations(native.package_name(), name, kinds, known_gaps),
    )
