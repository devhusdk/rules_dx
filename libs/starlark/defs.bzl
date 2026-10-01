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

def _shell_quote(s):
    """Single-quote a string for embedding in the generated runner script."""
    return "'" + s.replace("'", "'\\''") + "'"

def _parse_check(raw):
    """Parses one JSON check record into a struct."""
    record = json.decode(raw)
    kind = record["kind"] if "kind" in record else "equal"
    if kind == "equal":
        for key in ("name", "expected", "actual"):
            if key not in record:
                fail("starlark_test: check record is missing key '" + key + "': " + raw)
        return struct(
            kind = "equal",
            name = str(record["name"]),
            expected = str(record["expected"]),
            actual = str(record["actual"]),
        )
    elif kind == "true" or kind == "false":
        for key in ("name", "actual", "passed"):
            if key not in record:
                fail("starlark_test: check record is missing key '" + key + "': " + raw)
        return struct(
            kind = kind,
            name = str(record["name"]),
            actual = str(record["actual"]),
            passed = str(record["passed"]),
        )
    elif kind == "contains":
        for key in ("name", "haystack", "needle", "passed"):
            if key not in record:
                fail("starlark_test: check record is missing key '" + key + "': " + raw)
        return struct(
            kind = "contains",
            name = str(record["name"]),
            haystack = str(record["haystack"]),
            needle = str(record["needle"]),
            passed = str(record["passed"]),
        )
    elif kind == "match":
        for key in ("name", "value", "want", "passed"):
            if key not in record:
                fail("starlark_test: check record is missing key '" + key + "': " + raw)
        return struct(
            kind = "match",
            name = str(record["name"]),
            value = str(record["value"]),
            want = str(record["want"]),
            passed = str(record["passed"]),
        )
    else:
        fail("starlark_test: unknown check kind '" + kind + "': " + raw)

def _check_lines(checks):
    """Renders one shell assertion per check record, in declaration order."""
    lines = []
    for raw in checks:
        check = _parse_check(raw)
        if check.kind == "equal":
            lines.append(
                "check " + _shell_quote(check.name) + " " +
                _shell_quote(check.expected) + " " + _shell_quote(check.actual),
            )
        elif check.kind == "true":
            lines.append(
                "check_true " + _shell_quote(check.name) + " " +
                _shell_quote(check.actual) + " " + _shell_quote(check.passed),
            )
        elif check.kind == "false":
            lines.append(
                "check_false " + _shell_quote(check.name) + " " +
                _shell_quote(check.actual) + " " + _shell_quote(check.passed),
            )
        elif check.kind == "contains":
            lines.append(
                "check_contains " + _shell_quote(check.name) + " " +
                _shell_quote(check.haystack) + " " + _shell_quote(check.needle) + " " +
                _shell_quote(check.passed),
            )
        else:
            lines.append(
                "check_match " + _shell_quote(check.name) + " " +
                _shell_quote(check.value) + " " + _shell_quote(check.want) + " " +
                _shell_quote(check.passed),
            )
    return lines

def _file_check_lines(file_checks):
    """Renders one grep assertion per required substring."""
    lines = []
    for target in sorted(file_checks.keys(), key = lambda t: str(t.label)):
        want = file_checks[target]
        required = [line for line in want.split("\n") if line != ""]
        if len(required) == 0:
            fail("starlark_test: file_checks value is empty for " + str(target.label))
        for f in sorted(target.files.to_list(), key = lambda f: f.short_path):
            for i, needle in enumerate(required):
                lines.append(
                    "check_file " + _shell_quote(_display_label(target.label)) + " " +
                    _shell_quote(f.short_path) + " " +
                    _shell_quote(str(i + 1) + "/" + str(len(required))) + " " +
                    _shell_quote(needle),
                )
    return lines

_RUNNER_PRELUDE = [
    "#!/bin/sh",
    "# Generated by starlark_test. Do not edit.",
    "fail=0",
    "pass_count=0",
    "fail_count=0",
    "check() {",
    '    name="$1"; expected="$2"; actual="$3"',
    '    if [ "$expected" = "$actual" ]; then',
    '        echo "PASS: $name"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: $name"',
    '        echo "  expected: $expected"',
    '        echo "  actual:   $actual"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
    "check_true() {",
    '    name="$1"; actual="$2"; passed="$3"',
    '    if [ "$passed" = "True" ]; then',
    '        echo "PASS: $name"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: $name"',
    '        echo "  expected: True"',
    '        echo "  actual:   $actual"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
    "check_false() {",
    '    name="$1"; actual="$2"; passed="$3"',
    '    if [ "$passed" = "True" ]; then',
    '        echo "PASS: $name"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: $name"',
    '        echo "  expected: False"',
    '        echo "  actual:   $actual"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
    "check_contains() {",
    '    name="$1"; haystack="$2"; needle="$3"; passed="$4"',
    '    if [ "$passed" = "True" ]; then',
    '        echo "PASS: $name"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: $name"',
    '        echo "  haystack: $haystack"',
    '        echo "  missing:  $needle"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
    "check_match() {",
    '    name="$1"; value="$2"; want="$3"; passed="$4"',
    '    if [ "$passed" = "True" ]; then',
    '        echo "PASS: $name"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: $name"',
    '        echo "  value: $value"',
    '        echo "  missing substring: $want"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
    "rloc() {",
    '    p="$TEST_SRCDIR/$TEST_WORKSPACE/$1"',
    '    if [ -f "$p" ]; then',
    '        echo "$p"',
    "        return 0",
    "    fi",
    '    p="$0.runfiles/$TEST_WORKSPACE/$1"',
    '    if [ -f "$p" ]; then',
    '        echo "$p"',
    "        return 0",
    "    fi",
    '    m="$TEST_SRCDIR/MANIFEST"',
    '    [ -f "$m" ] || m="$0.runfiles/MANIFEST"',
    '    [ -f "$m" ] || m="$0.runfiles_manifest"',
    '    r=$(awk -v k="$TEST_WORKSPACE/$1" \'$1 == k { print $2; exit }\' "$m")',
    '    if [ -n "$r" ]; then',
    '        echo "$r"',
    "        return 0",
    "    fi",
    "    return 1",
    "}",
    "check_file() {",
    '    label="$1"; short_path="$2"; ordinal="$3"; want="$4"',
    "    path=$(rloc \"$short_path\")",
    '    if grep -q -F -e "$want" "$path"; then',
    '        echo "PASS: file $label contains substring $ordinal"',
    "        pass_count=$((pass_count + 1))",
    "    else",
    '        echo "FAIL: file $label is missing substring $ordinal"',
    '        echo "  substring: $want"',
    '        echo "  file: $path"',
    "        fail=1",
    "        fail_count=$((fail_count + 1))",
    "    fi",
    "}",
]

_RUNNER_EPILOGUE = [
    'echo "starlark_test: $pass_count passed, $fail_count failed"',
    "exit $fail",
]

def _target_is_windows(ctx):
    """Returns whether this target builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(
        ctx.attr._windows_os[platform_common.ConstraintValueInfo],
    )

def _write_windows_launcher(ctx, runner):
    """Writes the .bat launcher that runs the runner under Git's sh."""
    launcher = ctx.actions.declare_file(ctx.label.name + ".bat")
    key = ctx.workspace_name + "/" + runner.short_path
    body = "@echo off\r\n" + "setlocal ENABLEEXTENSIONS ENABLEDELAYEDEXPANSION\r\n" + \
           "set \"RUNNER=" + key.replace("\\", "/") + "\"\r\n" + \
           "if exist \"%TEST_SRCDIR%\\MANIFEST\" set \"M=%TEST_SRCDIR%\\MANIFEST\"\r\n" + \
           "if not defined M if exist \"%~f0.runfiles\\MANIFEST\" set \"M=%~f0.runfiles\\MANIFEST\"\r\n" + \
           "if not defined M set \"M=%~f0.runfiles_manifest\"\r\n" + \
           "for /F \"tokens=2* usebackq\" %%i in (`%SYSTEMROOT%\\system32\\findstr.exe /b /l /c:\"!RUNNER! \" \"%M%\"`) do set \"SCRIPT=%%i\"\r\n" + \
           "if not defined SCRIPT echo>&2 ERROR: !RUNNER! not found in the runfiles manifest & exit /b 1\r\n" + \
           "set \"PATH=C:\\Program Files\\Git\\usr\\bin;C:\\Program Files\\Git\\bin;%PATH%\"\r\n" + \
           "sh \"%SCRIPT:\\=/%\" %*\r\n" + \
           "exit /b %ERRORLEVEL%\r\n"
    ctx.actions.write(launcher, body, is_executable = True)
    return launcher

def _write_runner(ctx, body_lines, runfiles_files):
    """Writes the executable runner and stages its runfiles closure."""
    runner = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        runner,
        "\n".join(_RUNNER_PRELUDE + body_lines + _RUNNER_EPILOGUE) + "\n",
        is_executable = True,
    )
    transitive = []
    for target in ctx.attr.file_checks.keys():
        transitive.append(target[DefaultInfo].default_runfiles.files)
    runfiles = ctx.runfiles(
        files = runfiles_files + [runner],
        transitive_files = depset(transitive = transitive),
    )
    if _target_is_windows(ctx):
        return [DefaultInfo(executable = _write_windows_launcher(ctx, runner), runfiles = runfiles)]
    return [DefaultInfo(executable = runner, runfiles = runfiles)]

def _validate_common(mode, checks, subjects, file_checks):
    """Fails analysis when a test declares no evidence in any channel."""
    if len(checks) == 0 and len(subjects) == 0 and len(file_checks) == 0:
        fail("starlark_test (" + mode + " mode): no evidence: " +
             "provide checks, subjects, or file_checks")

def _file_check_files(file_checks):
    """Collects the staged files behind the file_checks mapping."""
    files = []
    for target in file_checks.keys():
        target_files = target.files.to_list()
        if len(target_files) == 0:
            fail("starlark_test: file_checks target has no files: " + str(target.label))
        files.extend(target_files)
    return files

def _load_test_impl(ctx):
    _validate_common("load", ctx.attr.checks, ctx.attr.subjects, ctx.attr.file_checks)
    if len(ctx.attr.checks) == 0:
        fail("starlark_test (load mode): checks must be non-empty: " +
             "load tests assert values computed while test files load")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (load mode): subjects must be empty: " +
             "load tests observe loading, not analysis")
    files = _file_check_files(ctx.attr.file_checks)
    body = _check_lines(ctx.attr.checks)
    body.extend(_file_check_lines(ctx.attr.file_checks))
    return _write_runner(ctx, body, files)

def _unit_test_impl(ctx):
    _validate_common("unit", ctx.attr.checks, ctx.attr.subjects, ctx.attr.file_checks)
    if len(ctx.attr.checks) == 0:
        fail("starlark_test (unit mode): checks must be non-empty: " +
             "unit tests assert pure function results")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (unit mode): subjects must be empty: " +
             "unit tests take no analysis subjects")
    files = _file_check_files(ctx.attr.file_checks)
    body = _check_lines(ctx.attr.checks)
    body.extend(_file_check_lines(ctx.attr.file_checks))
    return _write_runner(ctx, body, files)

def _observe_subjects(subjects):
    """Renders one observation block per subject target."""
    lines = []
    for target in sorted(subjects, key = lambda t: str(t.label)):
        lines.append("subject " + _display_label(target.label))
        info = target[DefaultInfo]
        for f in sorted(info.files.to_list(), key = lambda f: f.basename):
            lines.append("file " + f.basename)
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

def _observe_output_groups(subjects):
    """Renders dx_results output-group plus instrumented-files lines."""
    lines = []
    for target in sorted(subjects, key = lambda t: str(t.label)):
        if OutputGroupInfo in target:
            groups = target[OutputGroupInfo]
            if "dx_results" in groups:
                dx_files = sorted(groups["dx_results"].to_list(), key = lambda f: f.basename)
                lines.append("output_group " + _display_label(target.label) + " dx_results=" + (",".join([f.basename for f in dx_files]) if len(dx_files) > 0 else "(none)"))
            else:
                lines.append("output_group " + _display_label(target.label) + " dx_results=(none)")
        else:
            lines.append("output_group " + _display_label(target.label) + " dx_results=(none)")
        lines.append("instrumented " + _display_label(target.label) + "=" + str(InstrumentedFilesInfo in target))
    return lines

def _analysis_test_impl(ctx):
    _validate_common("analysis", ctx.attr.checks, ctx.attr.subjects, ctx.attr.file_checks)
    if len(ctx.attr.subjects) == 0:
        fail("starlark_test (analysis mode): subjects must be non-empty: " +
             "analysis tests observe subject targets")
    observed = _observe_subjects(ctx.attr.subjects)
    if ctx.attr.observe_output_groups:
        observed = observed + _observe_output_groups(ctx.attr.subjects)
    observations = ctx.actions.declare_file(ctx.label.name + "_observations.txt")
    ctx.actions.write(observations, "\n".join(observed) + "\n")
    body = _check_lines(ctx.attr.checks)
    body.extend(_file_check_lines(ctx.attr.file_checks))
    if ctx.attr.expected_observations != "":
        body.append("check " + _shell_quote("observations") + " " +
                    _shell_quote(ctx.attr.expected_observations.strip()) + " " +
                    _shell_quote("\n".join(observed)))
    body.append("echo '--- observations ---'")
    body.append("obs=$(rloc " + _shell_quote(observations.short_path) + ")")
    body.append("cat \"$obs\"")
    files = _file_check_files(ctx.attr.file_checks)
    files.append(observations)
    return _write_runner(ctx, body, files)

def _execution_test_impl(ctx):
    _validate_common("execution", ctx.attr.checks, ctx.attr.subjects, ctx.attr.file_checks)
    if len(ctx.attr.file_checks) == 0:
        fail("starlark_test (execution mode): file_checks must be non-empty: " +
             "execution tests assert on files read while the test runs")
    if len(ctx.attr.subjects) != 0:
        fail("starlark_test (execution mode): subjects must be empty: " +
             "execution tests read runfiles, not analysis subjects")
    files = _file_check_files(ctx.attr.file_checks)
    body = _check_lines(ctx.attr.checks)
    body.extend(_file_check_lines(ctx.attr.file_checks))
    return _write_runner(ctx, body, files)

_common_attrs = {
    "_windows_os": attr.label(default = "@platforms//os:windows"),
    "checks": attr.string_list(),
    "expected_observations": attr.string(
        default = "",
    ),
    "file_checks": attr.label_keyed_string_dict(
        allow_files = True,
    ),
    "observe_output_groups": attr.bool(
        default = False,
    ),
    "subjects": attr.label_list(
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

def starlark_test(name, mode, checks = [], subjects = [], expected_observations = "", file_checks = {}, observe_output_groups = False, **kwargs):
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
        file_checks = file_checks,
        observe_output_groups = observe_output_groups,
        **kwargs
    )
