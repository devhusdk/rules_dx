"""Behavioral fixtures for the workflow and evidence input validators and both aggregates."""

import importlib.util
import json
import os
import subprocess
import sys

from workflow_testing import ROOT, embedded_script

FIXTURE_DIR = "tools/ci/input_validation_fixtures"
AGGREGATE_FIXTURE_DIR = "tools/ci/aggregate_fixtures"
CI_GATE_FIXTURE_DIR = "tools/ci/ci_gate_fixtures"
WORKFLOW = ".github/workflows/reusable-consumer.yml"
CI_WORKFLOW = ".github/workflows/ci.yml"
EVIDENCE_WORKFLOW = ".github/workflows/evidence.yml"
VALIDATOR = "tools/ci/input_validation.py"
AGGREGATE_HEREDOC = "python3 - <<'EOF'"
AGGREGATE_MARKER = "EOF"
CI_AGGREGATE_HEREDOC = "python3 - <<'DX_AGGREGATE'"
CI_AGGREGATE_MARKER = "DX_AGGREGATE"
EVIDENCE_HEREDOC = "python3 - <<'DX_EVIDENCE_INPUTS'"
EVIDENCE_MARKER = "DX_EVIDENCE_INPUTS"
# Workflows whose run: blocks must read inputs through env, never through ${{ }}.
EXPRESSION_FREE_WORKFLOWS = (
    EVIDENCE_WORKFLOW,
    ".github/workflows/apple.yml",
)
INPUTS = (
    "DX_PLATFORMS",
    "DX_DISABLED_CHECKS",
    "DX_SCHEDULING_MODE",
    "DX_CODE_SCANNING_OPT_IN",
    "DX_MIN_COVERAGE",
    "DX_MIN_CHANGED_LINE_COVERAGE",
    "DX_INVOCATION_ID",
)
CHECKS = (
    "lint",
    "typecheck",
    "format",
    "generate",
    "security-audit",
    "license-audit",
    "test",
    "build",
    "coverage",
)
GATE_JOBS = (
    "dogfood",
    "docs",
    "devcontainer-check",
    "hermetic",
    "preflight",
    "workflow-check",
)
CONSUMER_JOBS = ("validate", "advisory-snapshots", *CHECKS)
RESULTS = ("success", "failure", "skipped", "cancelled")
PLAIN_OUTPUTS = (
    "artifact_suffix",
    "cache_saver",
    "disabled",
    "min_changed_line_coverage",
    "min_coverage",
    "snapshot_artifact",
)


def embedded_aggregate():
    """Returns the aggregate accounting the workflow runs, dedented to its own file."""
    return embedded_script(WORKFLOW, AGGREGATE_HEREDOC, AGGREGATE_MARKER)


def embedded_ci_aggregate():
    """Returns the outer gate accounting ci.yml runs, dedented to its own file."""
    return embedded_script(CI_WORKFLOW, CI_AGGREGATE_HEREDOC, CI_AGGREGATE_MARKER)


def fixture_names(directory):
    """Returns every fixture directory name, sorted."""
    return sorted(path.name for path in (ROOT / directory).iterdir() if path.is_dir())


def expectation(directory, case):
    """Returns the inputs, exit status, texts and outputs one fixture states."""
    lines = (ROOT / directory / case / "expect.txt").read_text(encoding="utf-8")
    inputs = {}
    status = None
    says = []
    outputs = {}
    for line in lines.splitlines():
        if line.startswith("input "):
            inputs = json.loads(line[len("input ") :])
        elif line.startswith("exit "):
            status = int(line[len("exit ") :])
        elif line.startswith("says "):
            says.append(line[len("says ") :])
        elif line.startswith("out "):
            key, _, value = line[len("out ") :].partition(" ")
            outputs[key] = json.loads(value)
    return inputs, status, says, outputs


def read_outputs(path):
    """Returns the job outputs one validator run appended, as a dict."""
    if not path.exists():
        return {}
    out = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, _, value = line.partition("=")
        out[key] = value if key in PLAIN_OUTPUTS else json.loads(value)
    return out


def run_script(tmp_path, name, script, inputs):
    """Runs one workflow script under the fixture inputs and returns status and text."""
    path = tmp_path / name
    path.write_text(script, encoding="utf-8")
    env = dict(os.environ)
    for name in INPUTS:
        env.pop(name, None)
    for key, value in inputs.items():
        env[key] = value if isinstance(value, str) else json.dumps(value)
    return subprocess.run(
        [sys.executable, str(path)],
        capture_output=True,
        cwd=tmp_path,
        encoding="utf-8",
        env=env,
    )


def run_validator(tmp_path, case, inputs):
    """Runs the validator over one fixture and returns status, text and outputs."""
    unknown = sorted(set(inputs) - set(INPUTS))
    assert not unknown, f"{case} passes {unknown}, which the validator never reads"
    outputs = tmp_path / f"{case}.outputs"
    done = run_script(
        tmp_path,
        "input_validation.py",
        (ROOT / VALIDATOR).read_text(encoding="utf-8"),
        dict(inputs, GITHUB_OUTPUT=str(outputs)),
    )
    return done.returncode, done.stdout + done.stderr, read_outputs(outputs)


def test_every_fixture_states_one_expected_outcome():
    names = fixture_names(FIXTURE_DIR)
    assert len(names) > 0, f"{FIXTURE_DIR} holds no fixture"
    for case in names:
        _, status, says, outputs = expectation(FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no validator text"
        assert set(outputs) <= {
            "platforms",
            "runners",
            "disabled",
            "cache_saver",
            "min_coverage",
            "min_changed_line_coverage",
            "snapshot_artifact",
            "artifact_suffix",
        }, f"{case} expects an unknown output {sorted(outputs)}"
        assert (status == 0) == bool(outputs), (
            f"{case} pairs status {status} with {outputs}"
        )


def test_fixtures_report_the_expected_diagnostic(tmp_path):
    for case in fixture_names(FIXTURE_DIR):
        inputs, status, says, _ = expectation(FIXTURE_DIR, case)
        code, output, _ = run_validator(tmp_path, case, inputs)
        assert code == status, f"{case} exited {code}, expected {status}: {output}"
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"


def test_fixtures_write_the_validated_outputs(tmp_path):
    for case in fixture_names(FIXTURE_DIR):
        inputs, status, _, outputs = expectation(FIXTURE_DIR, case)
        _, output, written = run_validator(tmp_path, case, inputs)
        if status == 0:
            assert written == outputs, (
                f"{case} wrote {written}, expected {outputs}: {output}"
            )
        else:
            assert written == {}, (
                f"{case} wrote {written} beside its rejection: {output}"
            )


def test_every_input_has_a_rejected_case():
    rejected = {}
    for case in fixture_names(FIXTURE_DIR):
        inputs, status, _, _ = expectation(FIXTURE_DIR, case)
        if status != 1:
            continue
        for name in inputs:
            rejected.setdefault(name, []).append(case)
    missing = [name for name in INPUTS if name not in rejected]
    assert not missing, f"no fixture rejects {missing}"


def test_every_aggregate_fixture_states_one_expected_outcome():
    names = fixture_names(AGGREGATE_FIXTURE_DIR)
    assert len(names) > 0, f"{AGGREGATE_FIXTURE_DIR} holds no fixture"
    for case in names:
        inputs, status, says, outputs = expectation(AGGREGATE_FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no aggregate text"
        assert not outputs, f"{case} expects job outputs from the aggregate: {outputs}"
        needs = inputs["NEEDS"]
        assert "validate" in needs, f"{case} drops the validate job: {needs}"
        assert set(needs) <= {"validate", "advisory-snapshots", *CHECKS}, (
            f"{case} needs {sorted(needs)}"
        )
        assert needs["validate"]["result"] in RESULTS, (
            f"{case} leaves the validate job undecided: {needs['validate']}"
        )
        assert all(
            needs[name]["result"] in RESULTS for name in needs if name != "validate"
        ), f"{case} holds an unknown job result: {needs}"


def test_aggregate_fixtures_report_the_expected_verdict(tmp_path):
    for case in fixture_names(AGGREGATE_FIXTURE_DIR):
        inputs, status, says, _ = expectation(AGGREGATE_FIXTURE_DIR, case)
        assert sorted(inputs) == ["DISABLED", "NEEDS"], (
            f"{case} passes {sorted(inputs)}"
        )
        done = run_script(tmp_path, "aggregate.py", embedded_aggregate(), inputs)
        output = done.stdout + done.stderr
        assert done.returncode == status, (
            f"{case} exited {done.returncode}, expected {status}: {output}"
        )
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"


def test_every_ci_gate_fixture_states_one_expected_outcome():
    names = fixture_names(CI_GATE_FIXTURE_DIR)
    assert len(names) > 0, f"{CI_GATE_FIXTURE_DIR} holds no fixture"
    for case in names:
        inputs, status, says, outputs = expectation(CI_GATE_FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no gate text"
        assert not outputs, f"{case} expects job outputs from the gate: {outputs}"
        assert sorted(inputs) == ["NEEDS"], f"{case} passes {sorted(inputs)}"
        needs = inputs["NEEDS"]
        assert 0 < len(needs) <= len(GATE_JOBS), f"{case} needs {sorted(needs)}"
        assert set(needs) <= set(GATE_JOBS), f"{case} needs {sorted(needs)}"
        for name in needs:
            assert needs[name]["result"] in RESULTS, (
                f"{case} holds an unknown job result: {needs}"
            )


def test_ci_gate_fixtures_report_the_expected_verdict(tmp_path):
    for case in fixture_names(CI_GATE_FIXTURE_DIR):
        inputs, status, says, _ = expectation(CI_GATE_FIXTURE_DIR, case)
        script = embedded_ci_aggregate()
        done = run_script(tmp_path, "ci_aggregate.py", script, inputs)
        output = done.stdout + done.stderr
        assert done.returncode == status, (
            f"{case} exited {done.returncode}, expected {status}: {output}"
        )
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"


def payload_all_success(inputs):
    """Returns whether every needed job reports success."""
    return all(value["result"] == "success" for value in inputs["NEEDS"].values())


def payload_reports(result):
    """Returns a check that some needed job reports exactly this result."""
    return lambda inputs: any(
        value["result"] == result for value in inputs["NEEDS"].values()
    )


def payload_disabled_and_skipped(inputs):
    """Returns whether an explicitly disabled check stays skipped."""
    return bool(inputs.get("DISABLED")) and payload_reports("skipped")(inputs)


def payload_skipped_undisabled(inputs):
    """Returns whether an undisabled check was skipped."""
    return not inputs.get("DISABLED") and payload_reports("skipped")(inputs)


def payload_drops(jobs):
    """Returns a check that the payload omits one declared job."""
    return lambda inputs: set(jobs) - set(inputs["NEEDS"])


def test_the_result_truth_table_covers_every_aggregate_state(tmp_path):
    table = {
        AGGREGATE_FIXTURE_DIR: (
            "aggregate.py",
            embedded_aggregate(),
            (
                ("all_checks_green", 0, payload_all_success),
                ("failing_check_fails", 1, payload_reports("failure")),
                ("cancelled_check_fails", 1, payload_reports("cancelled")),
                ("disabled_check_stays_green", 0, payload_disabled_and_skipped),
                ("skipped_without_disabled_fails", 1, payload_skipped_undisabled),
                ("missing_check_fails", 1, payload_drops(CONSUMER_JOBS)),
            ),
        ),
        CI_GATE_FIXTURE_DIR: (
            "ci_aggregate.py",
            embedded_ci_aggregate(),
            (
                ("all_green", 0, payload_all_success),
                ("failed_job_fails", 1, payload_reports("failure")),
                ("cancelled_job_fails", 1, payload_reports("cancelled")),
                ("skipped_job_fails", 1, payload_reports("skipped")),
                ("missing_job_fails", 1, payload_drops(GATE_JOBS)),
            ),
        ),
    }
    for directory, (name, script, rows) in table.items():
        for case, status, exhibits in rows:
            assert case in fixture_names(directory), f"{directory} lacks {case}"
            inputs, _, _, _ = expectation(directory, case)
            assert exhibits(inputs), f"{case} does not exhibit its named state"
            done = run_script(tmp_path, name, script, inputs)
            output = done.stdout + done.stderr
            assert done.returncode == status, (
                f"{case} exited {done.returncode}, expected {status}: {output}"
            )


def run_evidence(tmp_path, scope, platforms='["linux_x86_64"]', test_timeout="600"):
    """Runs the evidence input validator on one input set and returns status, text and outputs."""
    outputs = tmp_path / "evidence.outputs"
    done = run_script(
        tmp_path,
        "evidence_inputs.py",
        embedded_script(EVIDENCE_WORKFLOW, EVIDENCE_HEREDOC, EVIDENCE_MARKER),
        {
            "DX_SCOPE": scope,
            "DX_TASK": "test",
            "DX_PLATFORMS": platforms,
            "DX_DISPLAYS": "",
            "DX_TEST_TIMEOUT": test_timeout,
            "GITHUB_OUTPUT": str(outputs),
        },
    )
    written = outputs.read_text(encoding="utf-8") if outputs.exists() else None
    return done.returncode, done.stdout + done.stderr, written


def test_evidence_accepts_whitespace_separated_patterns(tmp_path):
    """Accepted scopes are written back as one line joined by single spaces."""
    cases = (
        ("//cli/cli/...", "//cli/cli/..."),
        (
            "//cli/process/... //tools/testing/process_probe/...",
            "//cli/process/... //tools/testing/process_probe/...",
        ),
        ("//a/...\t//b:t\n//c:*", "//a/... //b:t //c:*"),
    )
    for scope, expected in cases:
        status, output, written = run_evidence(tmp_path, scope)
        assert status == 0, f"{scope!r} exited {status}: {output}"
        assert written is not None, f"{scope!r} wrote no outputs"
        lines = written.splitlines()
        assert lines[0] == f"scope={expected}", f"{scope!r} wrote {lines[0]!r}"
        assert [line.partition("=")[0] for line in lines] == [
            "scope",
            "task",
            "platforms",
            "runners",
            "test_timeout",
        ], f"{scope!r} wrote extra keys: {lines}"
        (tmp_path / "evidence.outputs").unlink()


def test_evidence_rejects_bad_scopes(tmp_path):
    """Rejected scopes exit 1, name the bad pattern and write no outputs."""
    cases = (
        ("", "at least one target pattern"),
        (" \t\n", "at least one target pattern"),
        ("//a/... b/...", "'b/...'"),
        ("cli/...", "'cli/...'"),
        ("//$(id)", "'//$(id)'"),
        ("//a`id`", "'//a`id`'"),
        ("//a;id", "'//a;id'"),
        ("//a|b", "'//a|b'"),
        ('//"a"', "'//\"a\"'"),
        ("-//a", "'-//a'"),
        ("//a/... //$(id)", "'//$(id)'"),
    )
    for scope, says in cases:
        status, output, written = run_evidence(tmp_path, scope)
        assert status == 1, f"{scope!r} exited {status}: {output}"
        assert "evidence input rejected" in output, f"{scope!r}: {output}"
        assert says in output, f"{scope!r} does not say {says!r}: {output}"
        assert written is None, f"{scope!r} wrote outputs: {written!r}"


def test_evidence_passes_each_pattern_as_its_own_argument():
    """The run step splits the scope into an array instead of one quoted argument."""
    text = (ROOT / EVIDENCE_WORKFLOW).read_text(encoding="utf-8")
    assert 'read -ra patterns <<< "$DX_SCOPE"' in text
    assert '"${patterns[@]}"' in text
    assert '"$DX_SCOPE" 2>&1' not in text


def assert_rejected(case, status, output, written, says):
    """Asserts one rejected evidence input exited cleanly without writing outputs."""
    assert status == 1, f"{case!r} exited {status}: {output}"
    assert "evidence input rejected" in output, f"{case!r}: {output}"
    assert "Traceback" not in output, f"{case!r} raised: {output}"
    assert says in output, f"{case!r} does not say {says!r}: {output}"
    assert written is None, f"{case!r} wrote outputs: {written!r}"


def test_evidence_rejects_bad_platforms(tmp_path):
    """Bad platform lists exit 1 with a message instead of a traceback, writing nothing."""
    cases = (
        ('["linux_x86_64","linux_x86_64"]', "twice"),
        ("[1]", "JSON number"),
        ('[["linux_x86_64"]]', "JSON array"),
        ("{}", "JSON object"),
        ('"x"', "JSON string"),
        ("[]", "non-empty"),
        ('["plan9_x86_64"]', "unknown platforms"),
        ("[linux", "must be a JSON array"),
    )
    for platforms, says in cases:
        status, output, written = run_evidence(tmp_path, "//a/...", platforms=platforms)
        assert_rejected(platforms, status, output, written, says)


def test_evidence_rejects_bad_test_timeout(tmp_path):
    """A test_timeout that is not whole seconds exits 1 and writes nothing."""
    for test_timeout in ("", "1.5", "10s", "$(id)"):
        status, output, written = run_evidence(
            tmp_path, "//a/...", test_timeout=test_timeout
        )
        assert_rejected(test_timeout, status, output, written, "whole seconds")


def validator_platforms():
    """Returns the PLATFORMS table input_validation.py declares, without running its main."""
    spec = importlib.util.spec_from_file_location("input_validation", ROOT / VALIDATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.PLATFORMS


def test_evidence_runners_match_the_validator_platforms(tmp_path):
    """evidence.yml keeps its own platform table; it must map every platform like the validator."""
    status, output, written = run_evidence(tmp_path, "//a/...")
    assert status == 0, f"evidence exited {status}: {output}"
    assert written is not None, "evidence wrote no outputs"
    runners = [line for line in written.splitlines() if line.startswith("runners=")]
    assert len(runners) == 1, f"evidence wrote {len(runners)} runners lines: {written!r}"
    expected = validator_platforms()
    assert expected, f"{VALIDATOR} declares an empty PLATFORMS table"
    assert json.loads(runners[0].partition("=")[2]) == expected, (
        f"{EVIDENCE_WORKFLOW} PLATFORMS drifted from {VALIDATOR}"
    )


def run_blocks(text):
    """Yields (line number, line) for every line inside a run: block of a workflow."""
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        stripped = line.lstrip(" ")
        key = stripped[2:] if stripped.startswith("- ") else stripped
        if key.startswith("run:"):
            indent = len(line) - len(stripped)
            yield index + 1, line
            index += 1
            while index < len(lines):
                inner = lines[index]
                if inner.strip() and len(inner) - len(inner.lstrip(" ")) <= indent:
                    break
                yield index + 1, inner
                index += 1
            continue
        index += 1


def test_run_blocks_interpolate_no_expressions():
    """Shell steps read workflow values through env, so no ${{ }} reaches a run: block."""
    found = []
    for workflow in EXPRESSION_FREE_WORKFLOWS:
        text = (ROOT / workflow).read_text(encoding="utf-8")
        found += [
            f"{workflow}:{number}: {line.strip()}"
            for number, line in run_blocks(text)
            if "${{" in line
        ]
    assert not found, "move these expressions into the step env:\n" + "\n".join(found)
