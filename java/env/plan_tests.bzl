"""Focused Java environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //java/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=Hello.java
field has_sources=True
field has_tests=False
field source_count=1
field target=//java/tests/fixtures/hello:hello_lib
field test_source_count=0
field test_sources=
field transitive_source_count=1
field transitive_sources=libhello_lib_upstream-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//java/env:hello_lib_plan
aspect_field transitive_count=0
subject //java/env:hello_plan
file hello_plan.json
field direct_sources=Main.java
field has_sources=True
field has_tests=False
field source_count=1
field target=//java/tests/fixtures/hello:hello
field test_source_count=0
field test_sources=
field transitive_source_count=2
field transitive_sources=hello_upstream-src.jar,libhello_lib_upstream-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//java/env:hello_plan
aspect_field transitive_count=0
subject //java/env:hello_test_plan
file hello_test_plan.json
field direct_sources=HelloTest.java
field has_sources=True
field has_tests=True
field source_count=1
field target=//java/tests/fixtures/hello:hello_test
field test_source_count=2
field test_sources=HelloTest.java,hello_test_upstream-src.jar
field transitive_source_count=2
field transitive_sources=hello_test_upstream-src.jar,libhello_lib_upstream-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//java/env:hello_test_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
