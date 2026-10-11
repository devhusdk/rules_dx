"""Analysis tests pinning the frozen quality settings defaults."""

load("//libs/starlark:defs.bzl", "starlark_test")

_CANONICAL_PREFIX = "@@"  # buildifier: disable=canonical-repository

EXPECTED_OBSERVATIONS = """subject //config:settings_under_test
field fail_on=warning
field validate=False
field workspace={prefix}//quality:default_workspace_policy
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//config:settings_under_test
aspect_field transitive_count=0""".format(prefix = _CANONICAL_PREFIX)

def settings_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":settings_under_test"],
        expected_observations = EXPECTED_OBSERVATIONS,
    )

EXPECTED_WORKSPACE_POLICY_OBSERVATIONS = """subject //config:workspace_policy_under_test
field disabled.graphql.audit=False
field disabled.graphql.format=False
field disabled.graphql.lint=True
field disabled.graphql.typecheck=True
field disabled.html.audit=False
field disabled.html.format=False
field disabled.html.lint=True
field disabled.html.typecheck=True
field disabled.java.audit=False
field disabled.java.format=False
field disabled.java.lint=False
field disabled.java.typecheck=True
field disabled.javascript.audit=False
field disabled.javascript.format=False
field disabled.javascript.lint=False
field disabled.javascript.typecheck=True
field disabled.json.audit=False
field disabled.json.format=False
field disabled.json.lint=False
field disabled.json.typecheck=True
field disabled.kotlin.audit=False
field disabled.kotlin.format=False
field disabled.kotlin.lint=False
field disabled.kotlin.typecheck=True
field disabled.markdown.audit=False
field disabled.markdown.format=True
field disabled.markdown.lint=False
field disabled.markdown.typecheck=True
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=False
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field disabled.shell.audit=False
field disabled.shell.format=False
field disabled.shell.lint=False
field disabled.shell.typecheck=True
field disabled.starlark.audit=False
field disabled.starlark.format=False
field disabled.starlark.lint=False
field disabled.starlark.typecheck=True
field disabled.text.audit=False
field disabled.text.format=True
field disabled.text.lint=False
field disabled.text.typecheck=True
field disabled.toml.audit=False
field disabled.toml.format=False
field disabled.toml.lint=False
field disabled.toml.typecheck=True
field disabled.typescript.audit=False
field disabled.typescript.format=False
field disabled.typescript.lint=False
field disabled.typescript.typecheck=True
field family.graphql.audit=
field family.graphql.format=prettier
field family.graphql.lint=
field family.graphql.typecheck=
field family.html.audit=
field family.html.format=prettier
field family.html.lint=
field family.html.typecheck=
field family.java.audit=
field family.java.format=google_java_format
field family.java.lint=checkstyle,pmd,spotbugs
field family.java.typecheck=
field family.javascript.audit=
field family.javascript.format=biome
field family.javascript.lint=biome
field family.javascript.typecheck=
field family.json.audit=
field family.json.format=prettier
field family.json.lint=biome
field family.json.typecheck=
field family.kotlin.audit=
field family.kotlin.format=ktfmt
field family.kotlin.lint=ktlint
field family.kotlin.typecheck=
field family.markdown.audit=
field family.markdown.format=
field family.markdown.lint=markdown_check,vale
field family.markdown.typecheck=
field family.python.audit=
field family.python.format=ruff
field family.python.lint=pydoclint,ruff
field family.python.typecheck=ty
field family.rust.audit=
field family.rust.format=rustfmt
field family.rust.lint=clippy
field family.rust.typecheck=rustc
field family.shell.audit=
field family.shell.format=shfmt
field family.shell.lint=shellcheck
field family.shell.typecheck=
field family.starlark.audit=
field family.starlark.format=buildifier
field family.starlark.lint=buildifier
field family.starlark.typecheck=
field family.text.audit=
field family.text.format=
field family.text.lint=keep_sorted
field family.text.typecheck=
field family.toml.audit=
field family.toml.format=taplo
field family.toml.lint=taplo
field family.toml.typecheck=
field family.typescript.audit=
field family.typescript.format=biome
field family.typescript.lint=biome
field family.typescript.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=112
aspect_field has_subject=True
aspect_field subject_label=//config:workspace_policy_under_test
aspect_field transitive_count=0"""

def workspace_policy_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":workspace_policy_under_test"],
        expected_observations = EXPECTED_WORKSPACE_POLICY_OBSERVATIONS,
    )
