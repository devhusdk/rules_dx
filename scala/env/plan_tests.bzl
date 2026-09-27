"""Focused Scala environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //scala/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=Hello.scala
field has_sources=True
field has_tests=False
field source_count=1
field target=//scala/tests/fixtures/hello:hello_lib
field test_source_count=0
field test_sources=
field transitive_source_count=3
field transitive_sources=hello_lib_upstream-src.jar,scala-library-2.13.18-src.jar,scala-reflect-2.13.18-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//scala/env:hello_lib_plan
aspect_field transitive_count=0
subject //scala/env:hello_plan
file hello_plan.json
field direct_sources=Main.scala
field has_sources=True
field has_tests=False
field source_count=1
field target=//scala/tests/fixtures/hello:hello
field test_source_count=0
field test_sources=
field transitive_source_count=4
field transitive_sources=hello_lib_upstream-src.jar,hello_upstream-src.jar,scala-library-2.13.18-src.jar,scala-reflect-2.13.18-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//scala/env:hello_plan
aspect_field transitive_count=0
subject //scala/env:hello_test_plan
file hello_test_plan.json
field direct_sources=HelloTest.scala
field has_sources=True
field has_tests=True
field source_count=1
field target=//scala/tests/fixtures/hello:hello_test
field test_source_count=2
field test_sources=HelloTest.scala,hello_test_upstream-src.jar
field transitive_source_count=21
field transitive_sources=hello_lib_upstream-src.jar,hello_test_upstream-src.jar,scala-library-2.13.18-src.jar,scala-reflect-2.13.18-src.jar,scala-xml_2.13-2.1.0-src.jar,scalactic_2.13-3.2.19-src.jar,scalatest-compatible-3.2.19-src.jar,scalatest-core_2.13-3.2.19-src.jar,scalatest-diagrams_2.13-3.2.19-src.jar,scalatest-featurespec_2.13-3.2.19-src.jar,scalatest-flatspec_2.13-3.2.19-src.jar,scalatest-freespec_2.13-3.2.19-src.jar,scalatest-funspec_2.13-3.2.19-src.jar,scalatest-funsuite_2.13-3.2.19-src.jar,scalatest-matchers-core_2.13-3.2.19-src.jar,scalatest-mustmatchers_2.13-3.2.19-src.jar,scalatest-propspec_2.13-3.2.19-src.jar,scalatest-refspec_2.13-3.2.19-src.jar,scalatest-shouldmatchers_2.13-3.2.19-src.jar,scalatest-wordspec_2.13-3.2.19-src.jar,scalatest_2.13-3.2.19-src.jar
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//scala/env:hello_test_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
