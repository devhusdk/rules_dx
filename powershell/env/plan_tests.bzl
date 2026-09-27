"""Focused PowerShell environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //powershell/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=Greet.psd1,Greet.psm1
field has_sources=True
field source_count=2
field target=//powershell/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//powershell/env:hello_lib_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
