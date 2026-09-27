"""Focused JavaScript environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //javascript/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=hello.js
field has_npm=False
field has_store=False
field npm_source_count=0
field store_count=0
field target=//javascript/tests/fixtures/hello:hello_lib
field transitive_sources=hello.js
aspect_field aspect_seen=True
aspect_field field_count=7
aspect_field has_subject=True
aspect_field subject_label=//javascript/env:hello_lib_plan
aspect_field transitive_count=0
subject //javascript/env:helper_plan
file helper_plan.json
field direct_sources=helper.js
field has_npm=False
field has_store=False
field npm_source_count=0
field store_count=0
field target=//javascript/tests/fixtures/entries:helper
field transitive_sources=helper.js
aspect_field aspect_seen=True
aspect_field field_count=7
aspect_field has_subject=True
aspect_field subject_label=//javascript/env:helper_plan
aspect_field transitive_count=0
subject //javascript/env:main_plan
file main_plan.json
field direct_sources=main.js
field has_npm=False
field has_store=False
field npm_source_count=0
field store_count=0
field target=//javascript/tests/fixtures/entries:main
field transitive_sources=helper.js,main.js
aspect_field aspect_seen=True
aspect_field field_count=7
aspect_field has_subject=True
aspect_field subject_label=//javascript/env:main_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
