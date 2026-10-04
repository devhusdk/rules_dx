"""Behavioral fixtures for the consumer workflow input validator and aggregate."""

import json
import os
import pathlib
import subprocess
import sys

FIXTURE_DIR = "tools/ci/input_validation_fixtures"
AGGREGATE_FIXTURE_DIR = "tools/ci/aggregate_fixtures"
WORKFLOW = ".github/workflows/reusable-consumer.yml"
VALIDATOR = "tools/ci/input_validation.py"
VALIDATOR_HEREDOC = "python3 - <<'DX_INPUT_VALIDATION'"
VALIDATOR_MARKER = "DX_INPUT_VALIDATION"
AGGREGATE_HEREDOC = "python3 - <<'EOF'"
AGGREGATE_MARKER = "EOF"
INPUTS = (
    "DX_PLATFORMS",
    "DX_DISABLED_CHECKS",
    "DX_SCHEDULING_MODE",
    "DX_CODE_SCANNING_OPT_IN",
    "DX_MIN_COVERAGE",
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
PLAIN_OUTPUTS = ("disabled", "min_coverage")


def workspace_root():
    """Returns the runfiles root this test reads its own tree from."""
    return pathlib.Path(__file__).resolve().parents[2]


ROOT = workspace_root()


def embedded_script(opening, marker):
    """Returns one heredoc body from the workflow, dedented to its own file text."""
    lines = (ROOT / WORKFLOW).read_text(encoding="utf-8").splitlines()
    openings = [i for i, line in enumerate(lines) if line.strip() == opening]
    assert len(openings) == 1, (
        f"{WORKFLOW} runs {len(openings)} {marker} scripts, want 1"
    )
    base = len(lines[openings[0]]) - len(lines[openings[0]].lstrip(" "))
    body = []
    for line in lines[openings[0] + 1 :]:
        if line.strip() == marker:
            return "\n".join(body) + "\n"
        body.append(line[base:])
    raise AssertionError(f"{WORKFLOW} has no {marker} terminator")


def embedded_validator():
    """Returns the validator the workflow runs, dedented to its own file text."""
    return embedded_script(VALIDATOR_HEREDOC, VALIDATOR_MARKER)


def embedded_aggregate():
    """Returns the aggregate accounting the workflow runs, dedented to its own file."""
    return embedded_script(AGGREGATE_HEREDOC, AGGREGATE_MARKER)


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
        embedded_validator(),
        dict(inputs, GITHUB_OUTPUT=str(outputs)),
    )
    return done.returncode, done.stdout + done.stderr, read_outputs(outputs)


def test_the_workflow_runs_the_canonical_validator():
    canonical = (ROOT / VALIDATOR).read_text(encoding="utf-8")
    assert embedded_validator() == canonical, (
        f"{WORKFLOW} must run {VALIDATOR} verbatim, so one validator owns every input"
    )


def test_every_fixture_states_one_expected_outcome():
    names = fixture_names(FIXTURE_DIR)
    assert len(names) > 0, f"{FIXTURE_DIR} holds no fixture"
    for case in names:
        _, status, says, outputs = expectation(FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no validator text"
        assert set(outputs) <= {"platforms", "runners", "disabled", "min_coverage"}, (
            f"{case} expects an unknown output {sorted(outputs)}"
        )
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
        assert set(needs) == {"validate", *CHECKS}, f"{case} needs {sorted(needs)}"
        assert needs["validate"]["result"] in ("success", "failure"), (
            f"{case} leaves the validate job undecided: {needs['validate']}"
        )
        assert all(
            needs[name]["result"] in ("success", "failure", "skipped")
            for name in CHECKS
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
