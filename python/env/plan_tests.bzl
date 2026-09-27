"""Focused Python environment-plan tests."""

load("//env:plan_factory.bzl", _env_plan_tests = "env_plan_tests")

EXPECTED_ENV_PLAN_OBSERVATIONS = """subject //python/env:hello_lib_plan
file hello_lib_plan.json
field direct_sources=hello.py
field has_venv=False
field has_wheels=False
field imports=_main,_main/python/tests/fixtures/hello
field target=//python/tests/fixtures/hello:hello_lib
field transitive_sources=hello.py
field venv=
field wheel_count=0
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//python/env:hello_lib_plan
aspect_field transitive_count=0
subject //python/env:hello_plan
file hello_plan.json
field direct_sources=main.py
field has_venv=True
field has_wheels=False
field imports=_main,_main/python/tests/fixtures/hello
field target=//python/tests/fixtures/hello:hello
field transitive_sources=hello.py,main.py
field venv=python/tests/fixtures/hello/._hello_upstream.venv
field wheel_count=0
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//python/env:hello_plan
aspect_field transitive_count=0
subject //python/env:hello_test_plan
file hello_test_plan.json
field direct_sources=hello_test.py
field has_venv=True
field has_wheels=True
field imports=_main,_main/python/tests/fixtures/hello,aspect_rules_py+,aspect_rules_py++uv+project__hello,aspect_rules_py++uv+whl_install__hello__coverage__7_16_0/actual_install.install/lib/python3.12/site-packages,aspect_rules_py++uv+whl_install__hello__iniconfig__2_3_0/actual_install.install/lib/python3.12/site-packages,aspect_rules_py++uv+whl_install__hello__packaging__26_3/actual_install.install/lib/python3.12/site-packages,aspect_rules_py++uv+whl_install__hello__pluggy__1_6_0/actual_install.install/lib/python3.12/site-packages,aspect_rules_py++uv+whl_install__hello__pygments__2_21_0/actual_install.install/lib/python3.12/site-packages,aspect_rules_py++uv+whl_install__hello__pytest__9_1_1/actual_install.install/lib/python3.12/site-packages,aspect_rules_py+/py/private,aspect_rules_py+/py/private/launcher_env,aspect_rules_py+/py/private/pytest_shard
field target=//python/tests/fixtures/hello:hello_test
field transitive_sources=aspect_rules_py_launcher_env.py,hello.py,hello_test.py,pytest_main.py,pytest_shard.py
field venv=python/tests/fixtures/hello/._hello_test_upstream.venv
field wheel_count=6
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//python/env:hello_test_plan
aspect_field transitive_count=0
subject //python/env:main_bin_plan
file main_bin_plan.json
field direct_sources=
field has_venv=True
field has_wheels=False
field imports=_main,_main/python/tests/fixtures/entries
field target=//python/tests/fixtures/entries:main_bin
field transitive_sources=helper.py,main.py
field venv=python/tests/fixtures/entries/._main_bin_upstream.venv
field wheel_count=0
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//python/env:main_bin_plan
aspect_field transitive_count=0
subject //python/env:main_plan
file main_plan.json
field direct_sources=main.py
field has_venv=False
field has_wheels=False
field imports=_main,_main/python/tests/fixtures/entries
field target=//python/tests/fixtures/entries:main
field transitive_sources=helper.py,main.py
field venv=
field wheel_count=0
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//python/env:main_plan
aspect_field transitive_count=0"""

def env_plan_tests(name, subjects, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    _env_plan_tests(name, subjects, EXPECTED_ENV_PLAN_OBSERVATIONS, **kwargs)
