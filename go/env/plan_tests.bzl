"""Focused Go environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //go/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=hello.go
field has_sources=True
field has_tests=False
field source_count=1
field target=//go/tests/fixtures/hello:hello_lib
field test_source_count=0
field test_sources=
field transitive_source_count=1
field transitive_sources=hello.go
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//go/env:hello_lib_plan
aspect_field transitive_count=0
subject //go/env:hello_plan
file hello_plan.json
field direct_sources=main.go
field has_sources=True
field has_tests=False
field source_count=1
field target=//go/tests/fixtures/hello:hello
field test_source_count=0
field test_sources=
field transitive_source_count=2
field transitive_sources=hello.go,main.go
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//go/env:hello_plan
aspect_field transitive_count=0
subject //go/env:hello_test_plan
file hello_test_plan.json
field direct_sources=hello_test.go
field has_sources=True
field has_tests=True
field source_count=1
field target=//go/tests/fixtures/hello:hello_test
field test_source_count=1
field test_sources=hello_test.go
field transitive_source_count=10
field transitive_sources=coverdata.go,hello.go,hello_test.go,init.go,lcov.go,test2json.go,testmain.go,timeout.go,wrap.go,xml.go
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//go/env:hello_test_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
