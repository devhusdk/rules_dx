"""Unit and analysis tests for workspace policy and applicability."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":applicability.bzl", "capability_selection", "effective_classes", "selected_adapters")
load(":policy.bzl", "merge_configs", "merge_policy")

_FIXTURE_ADAPTERS = {
    "fmt-a": ["rust"],
    "lint-a": ["rust", "python"],
    "lint-b": ["rust"],
}

_FIXTURE_SECTION = {
    "format": ["fmt-a"],
    "lint": ["lint-a", "lint-b"],
    "typecheck": [],
}

def policy_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "capability_selection returns the lint selection",
                capability_selection(_FIXTURE_SECTION, "lint"),
                ["lint-a", "lint-b"],
            ),
            expect_equal(
                "capability_selection returns [] for an unselected capability",
                capability_selection(_FIXTURE_SECTION, "audit"),
                [],
            ),
            expect_equal(
                "selected_adapters preserves policy order",
                selected_adapters(["lint-b", "lint-a"], _FIXTURE_ADAPTERS),
                ["lint-b", "lint-a"],
            ),
            expect_equal(
                "selected_adapters deduplicates repeated IDs",
                selected_adapters(["lint-a", "lint-a"], _FIXTURE_ADAPTERS),
                ["lint-a"],
            ),
            expect_equal(
                "effective_classes intersects three sets sorted",
                effective_classes(
                    ["rust", "python", "markdown"],
                    ["rust", "python"],
                    ["python", "markdown"],
                ),
                ["python"],
            ),
            expect_equal(
                "effective_classes is empty without adapter support",
                effective_classes(["rust"], ["python"], ["rust", "python"]),
                [],
            ),
            expect_equal(
                "effective_classes is empty without policy authorization",
                effective_classes(["rust"], ["rust"], ["python"]),
                [],
            ),
            expect_equal(
                "merge_policy keeps the base without sections",
                merge_policy({"rust": "r", "python": "p"}, [], [], True),
                ({"rust": "r", "python": "p"}, ""),
            ),
            expect_equal(
                "merge_policy lets a section replace a base family",
                merge_policy({"rust": "r", "python": "p"}, [("python", "p2")], [], True),
                ({"rust": "r", "python": "p2"}, ""),
            ),
            expect_equal(
                "merge_policy adds a new family",
                merge_policy({"rust": "r"}, [("go", "g")], [], True),
                ({"rust": "r", "go": "g"}, ""),
            ),
            expect_equal(
                "merge_policy drops a disabled family",
                merge_policy({"rust": "r", "python": "p"}, [], ["rust"], True),
                ({"python": "p"}, ""),
            ),
            expect_equal(
                "merge_policy rejects disabled without extends",
                merge_policy({}, [], ["rust"], False)[1],
                "workspace_policy: 'disabled' needs 'extends'",
            ),
            expect_equal(
                "merge_policy rejects disabling a family the base lacks",
                merge_policy({"rust": "r"}, [], ["go"], True)[1],
                "workspace_policy: cannot disable family 'go', which the base policy does not have",
            ),
            expect_equal(
                "merge_policy rejects a family both disabled and provided",
                merge_policy({"rust": "r"}, [("rust", "r2")], ["rust"], True)[1],
                "workspace_policy: family 'rust' is both disabled and provided",
            ),
            expect_equal(
                "merge_policy rejects a family provided twice",
                merge_policy({}, [("go", "a"), ("go", "b")], [], False)[1],
                "workspace_policy: family 'go' is provided twice",
            ),
            expect_equal(
                "merge_configs keeps the base without entries",
                merge_configs({"ruff": "a"}, []),
                ({"ruff": "a"}, ""),
            ),
            expect_equal(
                "merge_configs lets an entry override the base",
                merge_configs({"ruff": "a"}, [("ruff", "b")]),
                ({"ruff": "b"}, ""),
            ),
            expect_equal(
                "merge_configs adds a tool",
                merge_configs({"ruff": "a"}, [("vale", "v")]),
                ({"ruff": "a", "vale": "v"}, ""),
            ),
            expect_equal(
                "merge_configs rejects a tool listed twice",
                merge_configs({}, [("ruff", "a"), ("ruff", "b")])[1],
                "workspace_policy: configs lists tool 'ruff' twice",
            ),
        ],
    )

EXPECTED_POLICY_OBSERVATIONS = """subject //quality:disable_policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=8
aspect_field has_subject=True
aspect_field subject_label=//quality:disable_policy_under_test
aspect_field transitive_count=0
subject //quality:extends_policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=False
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
field family.rust.audit=
field family.rust.format=fmt-a
field family.rust.lint=lint-a,lint-b
field family.rust.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=16
aspect_field has_subject=True
aspect_field subject_label=//quality:extends_policy_under_test
aspect_field transitive_count=0
subject //quality:override_policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=False
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-b
field family.python.typecheck=
field family.rust.audit=
field family.rust.format=fmt-a
field family.rust.lint=lint-a,lint-b
field family.rust.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=16
aspect_field has_subject=True
aspect_field subject_label=//quality:override_policy_under_test
aspect_field transitive_count=0
subject //quality:policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=False
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
field family.rust.audit=
field family.rust.format=fmt-a
field family.rust.lint=lint-a,lint-b
field family.rust.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=16
aspect_field has_subject=True
aspect_field subject_label=//quality:policy_under_test
aspect_field transitive_count=0"""

def policy_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":policy_under_test",
            ":extends_policy_under_test",
            ":override_policy_under_test",
            ":disable_policy_under_test",
        ],
        expected_observations = EXPECTED_POLICY_OBSERVATIONS,
    )

EXPECTED_PROFILE_OBSERVATIONS = """subject //quality:generated_policy_under_test
field config.biome=quality/tiers/biome/standard.json
field config.buildifier=quality/tiers/buildifier/standard.json
field config.checkstyle=quality/tiers/checkstyle/standard.xml
field config.eslint=quality/tiers/eslint/standard.mjs
field config.graphql_eslint=quality/tiers/graphql_eslint/standard.json
field config.ruff=quality/tiers/ruff/strict.toml
field config.sqlfluff=quality/tiers/sqlfluff/standard.cfg
field config.ty=quality/tiers/ty/standard.toml
field config.vale=quality/corpus_vale.ini
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
field profile.python=strict
aspect_field aspect_seen=True
aspect_field field_count=18
aspect_field has_subject=True
aspect_field subject_label=//quality:generated_policy_under_test
aspect_field transitive_count=0
"""

def policy_profile_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":generated_policy_under_test"],
        expected_observations = EXPECTED_PROFILE_OBSERVATIONS,
    )
