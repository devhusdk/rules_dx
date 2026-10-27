"""Project-owned Starlark testing facade."""

load("//libs/starlark:canonical.bzl", "strip_canonical")

DxSubjectInfo = provider(
    doc = "Analysis observations a subject rule exposes to starlark_test.",
    fields = {
        "fields": "Observations about the subject.",
    },
)

DxAspectInfo = provider(
    doc = "Aspect observations derived without subject cooperation.",
    fields = {
        "fields": "Observations the aspect saw.",
    },
)

DxConfigInfo = provider(
    doc = "Configuration observations a subject rule exposes to starlark_test.",
    fields = {
        "fields": "Configuration observations.",
    },
)

def _dx_aspect_note_impl(target, ctx):
    """Derives one aspect note without subject cooperation."""
    fields = {
        "aspect_seen": "True",
        "subject_label": _display_label(target.label),
    }
    if DxSubjectInfo in target:
        fields["field_count"] = str(len(target[DxSubjectInfo].fields.keys()))
        fields["has_subject"] = "True"
    else:
        fields["field_count"] = "0"
        fields["has_subject"] = "False"
    seen = []
    for dep in getattr(ctx.rule.attr, "deps", []):
        if DxAspectInfo in dep:
            label = dep[DxAspectInfo].fields.get("subject_label", "")
            if label != "":
                seen.append(label)
    if len(seen) > 0:
        fields["transitive"] = ",".join(sorted(seen))
    fields["transitive_count"] = str(len(seen))
    return [DxAspectInfo(fields = fields)]

dx_aspect_note = aspect(
    implementation = _dx_aspect_note_impl,
    doc = "Derives one aspect note for a target.",
    attr_aspects = ["deps"],
)

def expect_equal(name, actual, expected):
    """Builds one equality-check record as a JSON string."""
    return json.encode({
        "actual": actual,
        "expected": expected,
        "kind": "equal",
        "name": name,
    })

def expect_true(name, actual):
    """Builds one boolean-true record."""
    return json.encode({
        "actual": actual,
        "kind": "true",
        "name": name,
        "passed": actual == True,
    })

def expect_false(name, actual):
    """Builds one boolean-false record."""
    return json.encode({
        "actual": actual,
        "kind": "false",
        "name": name,
        "passed": actual == False,
    })

def expect_contains(name, haystack, needle):
    """Builds one membership record (string substring, list/tuple element, dict key)."""
    haystack_type = type(haystack)
    if haystack_type == "string":
        if type(needle) != "string":
            fail("expect_contains: needle must be string when haystack is string, got " + type(needle))
        passed = needle in haystack
    elif haystack_type == "list" or haystack_type == "tuple":
        passed = needle in haystack
    elif haystack_type == "dict":
        passed = needle in haystack
    else:
        fail("expect_contains: haystack must be string, list, tuple, or dict, got " + haystack_type)
    return json.encode({
        "haystack": haystack,
        "kind": "contains",
        "name": name,
        "needle": needle,
        "passed": passed,
    })

def expect_match(name, value, want):
    """Builds one stringified-substring record."""
    if type(want) != "string":
        fail("expect_match: want must be string, got " + type(want))
    return json.encode({
        "kind": "match",
        "name": name,
        "passed": want in str(value),
        "value": value,
        "want": want,
    })

def _display_label(label):
    """Renders a label for observations and diagnostics."""
    return strip_canonical(str(label))

def display_label(label):
    """Renders a label with the canonical-repository marker stripped."""
    return _display_label(label)

def _require_keys(record, keys, raw):
    """Fails analysis when a check record lacks one of the required keys."""
    for key in keys:
        if key not in record:
            fail("starlark_test: check record is missing key '" + key + "': " + raw)

def _harness_check_record(raw):
    """Normalizes one JSON check record into a manifest entry with string fields."""
    record = json.decode(raw)
    kind = record["kind"] if "kind" in record else "equal"
    if kind == "equal":
        _require_keys(record, ("name", "expected", "actual"), raw)
        return {
            "actual": str(record["actual"]),
            "expected": str(record["expected"]),
            "kind": "equal",
            "name": str(record["name"]),
        }
    if kind == "true" or kind == "false":
        _require_keys(record, ("name", "actual", "passed"), raw)
        return {
            "actual": str(record["actual"]),
            "kind": kind,
            "name": str(record["name"]),
            "passed": str(record["passed"]),
        }
    if kind == "contains":
        _require_keys(record, ("name", "haystack", "needle", "passed"), raw)
        return {
            "haystack": str(record["haystack"]),
            "kind": "contains",
            "name": str(record["name"]),
            "needle": str(record["needle"]),
            "passed": str(record["passed"]),
        }
    if kind == "match":
        _require_keys(record, ("name", "value", "want", "passed"), raw)
        return {
            "kind": "match",
            "name": str(record["name"]),
            "passed": str(record["passed"]),
            "value": str(record["value"]),
            "want": str(record["want"]),
        }
    fail("starlark_test: unknown check kind '" + kind + "': " + raw)

def _target_is_windows(ctx):
    """Returns whether this target builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(
        ctx.attr._windows_os[platform_common.ConstraintValueInfo],
    )

def _validate_common(mode, ctx):
    """Fails analysis when a test declares no evidence in any channel."""
    attr = ctx.attr
    if len(attr.checks) == 0 and len(attr.subjects) == 0 and len(attr.file_checks) == 0 and len(attr.file_blocks) == 0:
        fail("starlark_test (" + mode + " mode): no evidence: " +
             "provide checks, subjects, file_checks, or file_blocks")

_HARNESS_MANIFEST_VERSION = 2

_HARNESS_MANIFEST_ENV = "STARLARK_HARNESS_MANIFEST"

def _harness_runfiles_key(ctx, f):
    """Returns the runfiles key naming one staged file."""
    short_path = f.short_path
    if short_path.startswith("../"):
        return short_path[3:]
    return ctx.workspace_name + "/" + short_path

def _file_check_files(target):
    """Returns one file_checks target's files, failing analysis when it has none."""
    target_files = target.files.to_list()
    if len(target_files) == 0:
        fail("starlark_test: file_checks target has no files: " + str(target.label))
    return sorted(target_files, key = lambda f: f.short_path)

def _harness_file_entries(ctx):
    """Renders one manifest file entry per staged file behind file_checks and file_blocks."""
    entries = []
    staged = []
    for target in sorted(ctx.attr.file_checks.keys(), key = lambda t: str(t.label)):
        files = _file_check_files(target)
        want = ctx.attr.file_checks[target]
        required = [line for line in want.split("\n") if line != ""]
        if len(required) == 0:
            fail("starlark_test: file_checks value is empty for " + str(target.label))
        for f in files:
            entries.append({
                "key": _harness_runfiles_key(ctx, f),
                "label": _display_label(target.label),
                "needles": required,
            })
            staged.append(f)
    for target in sorted(ctx.attr.file_blocks.keys(), key = lambda t: str(t.label)):
        files = sorted(target.files.to_list(), key = lambda f: f.short_path)
        blocks = json.decode(ctx.attr.file_blocks[target])
        if type(blocks) != "list" or len(blocks) == 0:
            fail("starlark_test: file_blocks value is empty for " + str(target.label))
        for block in blocks:
            if type(block) != "string":
                fail("starlark_test: file_blocks block is not a string for " + str(target.label))
            if block == "":
                fail("starlark_test: file_blocks block is empty for " + str(target.label))
        for f in files:
            entries.append({
                "key": _harness_runfiles_key(ctx, f),
                "label": _display_label(target.label),
                "needles": blocks,
            })
            staged.append(f)
    return (entries, staged)

def _write_harness(ctx, records, observations):
    """Writes the v2 manifest and runs it through the native harness binary."""
    (entries, staged) = _harness_file_entries(ctx)
    manifest = ctx.actions.declare_file(ctx.label.name + "_starlark_manifest.json")
    ctx.actions.write(
        manifest,
        json.encode({
            "checks": records,
            "files": entries,
            "name": _display_label(ctx.label),
            "observations": observations,
            "version": _HARNESS_MANIFEST_VERSION,
        }) + "\n",
    )
    harness = ctx.executable._harness
    if _target_is_windows(ctx):
        executable = ctx.actions.declare_file(ctx.label.name + ".exe")
    else:
        executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = harness, is_executable = True)
    transitive = []
    for target in ctx.attr.file_checks.keys() + ctx.attr.file_blocks.keys():
        transitive.append(target[DefaultInfo].default_runfiles.files)
    runfiles = ctx.runfiles(
        files = staged + [manifest],
        transitive_files = depset(transitive = transitive),
    )
    runfiles = runfiles.merge(ctx.attr._harness[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = executable, runfiles = runfiles),
        RunEnvironmentInfo({_HARNESS_MANIFEST_ENV: _harness_runfiles_key(ctx, manifest)}),
        OutputGroupInfo(starlark_manifest = depset([manifest])),
    ]

def _check_records(ctx):
    """Normalizes every declared check record in declaration order."""
    return [_harness_check_record(raw) for raw in ctx.attr.checks]

def _load_test_impl(ctx):
    _validate_common("load", ctx)
    if len(ctx.attr.checks) == 0:
        fail("starlark_test (load mode): checks must be non-empty: " +
             "load tests assert values computed while test files load")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (load mode): subjects must be empty: " +
             "load tests observe loading, not analysis")
    return _write_harness(ctx, _check_records(ctx), [])

def _unit_test_impl(ctx):
    _validate_common("unit", ctx)
    if len(ctx.attr.checks) == 0:
        fail("starlark_test (unit mode): checks must be non-empty: " +
             "unit tests assert pure function results")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (unit mode): subjects must be empty: " +
             "unit tests take no analysis subjects")
    return _write_harness(ctx, _check_records(ctx), [])

def _observed_basename(name):
    """Renders a basename without the Windows executable suffix."""
    return name[:-4] if name.endswith(".exe") else name

def _observe_subjects(subjects):
    """Renders one observation block per subject target."""
    lines = []
    for target in sorted(subjects, key = lambda t: str(t.label)):
        lines.append("subject " + _display_label(target.label))
        info = target[DefaultInfo]
        for f in sorted(info.files.to_list(), key = lambda f: f.basename):
            lines.append("file " + _observed_basename(f.basename))
        if DxSubjectInfo in target:
            fields = target[DxSubjectInfo].fields
            for key in sorted(fields.keys()):
                lines.append("field " + key + "=" + fields[key])
        if DxAspectInfo in target:
            aspect_fields = target[DxAspectInfo].fields
            for key in sorted(aspect_fields.keys()):
                lines.append("aspect_field " + key + "=" + aspect_fields[key])
        if DxConfigInfo in target:
            config_fields = target[DxConfigInfo].fields
            for key in sorted(config_fields.keys()):
                lines.append("config_field " + key + "=" + config_fields[key])
    return lines

def _analysis_test_impl(ctx):
    _validate_common("analysis", ctx)
    if len(ctx.attr.subjects) == 0:
        fail("starlark_test (analysis mode): subjects must be non-empty: " +
             "analysis tests observe subject targets")
    observed = _observe_subjects(ctx.attr.subjects)
    records = _check_records(ctx)
    if ctx.attr.expected_observations != "":
        expected = ctx.attr.expected_observations
        if _target_is_windows(ctx) and ctx.attr.windows_expected_observations != "":
            expected = ctx.attr.windows_expected_observations
        records.append({
            "actual": "\n".join(observed),
            "expected": expected.strip(),
            "kind": "equal",
            "name": "observations",
        })
    return _write_harness(ctx, records, observed)

def _execution_test_impl(ctx):
    _validate_common("execution", ctx)
    if len(ctx.attr.file_checks) == 0 and len(ctx.attr.file_blocks) == 0:
        fail("starlark_test (execution mode): file_checks or file_blocks must be non-empty: " +
             "execution tests assert on files read while the test runs")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (execution mode): subjects must be empty: " +
             "execution tests read runfiles, not analysis subjects")
    return _write_harness(ctx, _check_records(ctx), [])

_common_attrs = {
    "_harness": attr.label(
        cfg = "target",
        default = Label("//tools/testing:starlark_harness"),
        executable = True,
    ),
    "_windows_os": attr.label(default = "@platforms//os:windows"),
    "checks": attr.string_list(
        doc = "JSON check records asserting expected against actual values.",
    ),
    "expected_observations": attr.string(
        doc = "Expected observation text compared with the observed subject data.",
        default = "",
    ),
    "windows_expected_observations": attr.string(
        doc = "Expected observation text used instead on Windows targets.",
        default = "",
    ),
    "file_blocks": attr.label_keyed_string_dict(
        doc = "Map from file targets to JSON lists of multi-line blocks each file must contain.",
        allow_files = True,
    ),
    "file_checks": attr.label_keyed_string_dict(
        doc = "Map from file targets to newline separated substrings each file must contain.",
        allow_files = True,
    ),
    "subjects": attr.label_list(
        doc = "Analysis subject targets whose fields and files are observed.",
        aspects = [dx_aspect_note],
    ),
}

_starlark_load_test = rule(
    implementation = _load_test_impl,
    test = True,
    attrs = _common_attrs,
)

_starlark_unit_test = rule(
    implementation = _unit_test_impl,
    test = True,
    attrs = _common_attrs,
)

_starlark_analysis_test = rule(
    implementation = _analysis_test_impl,
    test = True,
    attrs = _common_attrs,
)

_starlark_execution_test = rule(
    implementation = _execution_test_impl,
    test = True,
    attrs = _common_attrs,
)

_MODES = {
    "analysis": _starlark_analysis_test,
    "execution": _starlark_execution_test,
    "load": _starlark_load_test,
    "unit": _starlark_unit_test,
}

def starlark_test(name, mode, checks = [], subjects = [], expected_observations = "", windows_expected_observations = "", file_checks = {}, file_blocks = {}, **kwargs):
    """Instantiates one test target in the given mode."""
    if mode not in _MODES:
        fail("starlark_test: unknown mode '" + mode + "': want one of " +
             ", ".join(sorted(_MODES.keys())))
    kwargs.setdefault("size", "small")
    _MODES[mode](
        name = name,
        checks = checks,
        subjects = subjects,
        expected_observations = expected_observations,
        windows_expected_observations = windows_expected_observations,
        file_checks = file_checks,
        file_blocks = {label: json.encode(blocks) for label, blocks in file_blocks.items()},
        **kwargs
    )
