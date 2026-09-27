"""Focused Ruby environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //ruby/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=hello.rb
field has_sources=True
field source_count=1
field target=//ruby/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//ruby/env:hello_lib_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
