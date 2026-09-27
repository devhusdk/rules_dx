"""Focused Astro environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //astro/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=Hello.astro
field has_npm=False
field npm_source_count=0
field target=//astro/tests/fixtures/hello:hello_lib
field transitive_sources=Hello.astro
aspect_field aspect_seen=True
aspect_field field_count=5
aspect_field has_subject=True
aspect_field subject_label=//astro/env:hello_lib_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
