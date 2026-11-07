"""Unit and analysis tests for workspace policy and applicability."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":applicability.bzl", "capability_selection", "effective_classes", "selected_adapters")
load(":policy.bzl", "merge_configs", "merge_policy")
load("//quality/private:generated_policy.bzl", "generated_families_error")

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
                "generated_families_error accepts capability, disabled and tier keys",
                generated_families_error("p", {"python": {"disabled": ["audit"], "lint": ["ruff"], "tier": "strict"}}),
                "",
            ),
            expect_equal(
                "generated_families_error rejects an unknown spec key",
                generated_families_error("p", {"python": {"lints": ["ruff"]}}),
                "dx_generated_policy (p): family 'python' has unknown key 'lints'; use one of lint, typecheck, format, audit, disabled, tier",
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
field config.ansible_lint=quality/tiers/ansible_lint/standard.yaml
field config.biome=quality/tiers/biome/standard.json
field config.buf=quality/tiers/buf/standard.yaml
field config.buildifier=quality/tiers/buildifier/standard.json
field config.cfn_lint=quality/tiers/cfn_lint/standard.yaml
field config.checkmake=quality/tiers/checkmake/standard.ini
field config.checkstyle=quality/tiers/checkstyle/standard.xml
field config.clang_format=quality/tiers/clang_format/standard.clang-format
field config.clang_tidy=quality/tiers/clang_tidy/standard.clang-tidy
field config.cmakelint=quality/tiers/cmakelint/standard.cmakelintrc
field config.cppcheck=quality/tiers/cppcheck/standard.txt
field config.credo=quality/tiers/credo/standard.exs
field config.dart=quality/tiers/dart/standard.yaml
field config.detekt=quality/tiers/detekt/standard.yml
field config.elvis=quality/tiers/elvis/standard.config
field config.eslint=quality/tiers/eslint/standard.mjs
field config.ffmt=quality/tiers/ffmt/standard.toml
field config.fortitude=quality/tiers/fortitude/standard.toml
field config.fsharplint=quality/tiers/fsharplint/standard.json
field config.gersemi=quality/tiers/gersemi/standard.yaml
field config.gplint=quality/tiers/gplint/standard.json
field config.graphql_eslint=quality/tiers/graphql_eslint/standard.json
field config.hadolint=quality/tiers/hadolint/standard.yaml
field config.hlint=quality/tiers/hlint/standard.yaml
field config.luals=quality/tiers/luals/standard.json
field config.lychee=quality/tiers/lychee/standard.toml
field config.markdown_check=quality/tiers/markdown_check/standard.txt
field config.mbake=quality/tiers/mbake/standard.toml
field config.perlcritic=quality/tiers/perlcritic/standard.perlcriticrc
field config.perltidy=quality/tiers/perltidy/standard.perltidyrc
field config.phpcs=quality/tiers/phpcs/standard.xml
field config.phpstan=quality/tiers/phpstan/standard.neon
field config.pmd=quality/tiers/pmd/standard.xml
field config.psscriptanalyzer=quality/tiers/psscriptanalyzer/standard.psd1
field config.pydoclint=quality/tiers/pydoclint/standard.toml
field config.qmlformat=quality/tiers/qmlformat/standard.qmlformat.ini
field config.qmllint=quality/tiers/qmllint/standard.qmllint.ini
field config.redocly=quality/tiers/redocly/standard.yaml
field config.regal=quality/tiers/regal/standard.yaml
field config.roslyn=quality/tiers/roslyn/standard.globalconfig
field config.rubocop=quality/tiers/rubocop/standard.yml
field config.ruff=quality/tiers/ruff/strict.toml
field config.rustfmt=quality/tiers/rustfmt/standard.toml
field config.scalafix=quality/tiers/scalafix/standard.conf
field config.selene=quality/tiers/selene/standard.toml
field config.shellcheck=quality/tiers/shellcheck/standard.shellcheckrc
field config.sqlfluff=quality/tiers/sqlfluff/standard.cfg
field config.staticcheck=quality/tiers/staticcheck/standard.conf
field config.stylelint=quality/tiers/stylelint/standard.json
field config.stylua=quality/tiers/stylua/standard.toml
field config.swift_format=quality/tiers/swift_format/standard.json
field config.swift_lint=quality/tiers/swift_lint/standard.json
field config.taplo=quality/tiers/taplo/standard.toml
field config.tflint=quality/tiers/tflint/standard.hcl
field config.trivy_config=quality/tiers/trivy_config/standard.txt
field config.ty=quality/tiers/ty/standard.toml
field config.typos=quality/tiers/typos/standard.toml
field config.vale=quality/corpus_vale.ini
field config.yamlfmt=quality/tiers/yamlfmt/standard.yaml
field config.yamllint=quality/tiers/yamllint/standard.yaml
field config.zizmor=quality/tiers/zizmor/standard.txt
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
field tier.python=strict
aspect_field aspect_seen=True
aspect_field field_count=70
aspect_field has_subject=True
aspect_field subject_label=//quality:generated_policy_under_test
aspect_field transitive_count=0
subject //quality:merged_generated_policy_under_test
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field disabled.rust.audit=True
field disabled.rust.format=False
field disabled.rust.lint=False
field disabled.rust.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=ruff
field family.python.typecheck=
field family.rust.audit=
field family.rust.format=rustfmt
field family.rust.lint=clippy,typos
field family.rust.typecheck=rustc
aspect_field aspect_seen=True
aspect_field field_count=16
aspect_field has_subject=True
aspect_field subject_label=//quality:merged_generated_policy_under_test
aspect_field transitive_count=0
subject //quality:strict_generated_policy_under_test
field config.ansible_lint=quality/tiers/ansible_lint/strict.yaml
field config.biome=quality/tiers/biome/strict.json
field config.buf=quality/tiers/buf/strict.yaml
field config.buildifier=quality/tiers/buildifier/strict.json
field config.cfn_lint=quality/tiers/cfn_lint/strict.yaml
field config.checkmake=quality/tiers/checkmake/strict.ini
field config.checkstyle=quality/tiers/checkstyle/strict.xml
field config.clang_format=quality/tiers/clang_format/strict.clang-format
field config.clang_tidy=quality/tiers/clang_tidy/strict.clang-tidy
field config.cmakelint=quality/tiers/cmakelint/strict.cmakelintrc
field config.cppcheck=quality/tiers/cppcheck/strict.txt
field config.credo=quality/tiers/credo/strict.exs
field config.dart=quality/tiers/dart/strict.yaml
field config.detekt=quality/tiers/detekt/strict.yml
field config.elvis=quality/tiers/elvis/strict.config
field config.eslint=quality/tiers/eslint/strict.mjs
field config.ffmt=quality/tiers/ffmt/strict.toml
field config.fortitude=quality/tiers/fortitude/strict.toml
field config.fsharplint=quality/tiers/fsharplint/strict.json
field config.gersemi=quality/tiers/gersemi/strict.yaml
field config.gplint=quality/tiers/gplint/strict.json
field config.graphql_eslint=quality/tiers/graphql_eslint/strict.json
field config.hadolint=quality/tiers/hadolint/strict.yaml
field config.hlint=quality/tiers/hlint/strict.yaml
field config.luals=quality/tiers/luals/strict.json
field config.lychee=quality/tiers/lychee/strict.toml
field config.markdown_check=quality/tiers/markdown_check/strict.txt
field config.mbake=quality/tiers/mbake/strict.toml
field config.perlcritic=quality/tiers/perlcritic/strict.perlcriticrc
field config.perltidy=quality/tiers/perltidy/strict.perltidyrc
field config.phpcs=quality/tiers/phpcs/strict.xml
field config.phpstan=quality/tiers/phpstan/strict.neon
field config.pmd=quality/tiers/pmd/strict.xml
field config.psscriptanalyzer=quality/tiers/psscriptanalyzer/strict.psd1
field config.pydoclint=quality/tiers/pydoclint/strict.toml
field config.qmlformat=quality/tiers/qmlformat/strict.qmlformat.ini
field config.qmllint=quality/tiers/qmllint/strict.qmllint.ini
field config.redocly=quality/tiers/redocly/strict.yaml
field config.regal=quality/tiers/regal/strict.yaml
field config.roslyn=quality/tiers/roslyn/strict.globalconfig
field config.rubocop=quality/tiers/rubocop/strict.yml
field config.ruff=quality/tiers/ruff/strict.toml
field config.rustfmt=quality/tiers/rustfmt/strict.toml
field config.scalafix=quality/tiers/scalafix/strict.conf
field config.selene=quality/tiers/selene/strict.toml
field config.shellcheck=quality/tiers/shellcheck/strict.shellcheckrc
field config.sqlfluff=quality/tiers/sqlfluff/strict.cfg
field config.staticcheck=quality/tiers/staticcheck/strict.conf
field config.stylelint=quality/tiers/stylelint/strict.json
field config.stylua=quality/tiers/stylua/strict.toml
field config.swift_format=quality/tiers/swift_format/strict.json
field config.swift_lint=quality/tiers/swift_lint/strict.json
field config.taplo=quality/tiers/taplo/strict.toml
field config.tflint=quality/tiers/tflint/strict.hcl
field config.trivy_config=quality/tiers/trivy_config/strict.txt
field config.ty=quality/tiers/ty/strict.toml
field config.typos=quality/tiers/typos/strict.toml
field config.vale=quality/tiers/vale/strict.ini
field config.yamlfmt=quality/tiers/yamlfmt/strict.yaml
field config.yamllint=quality/tiers/yamllint/strict.yaml
field config.zizmor=quality/tiers/zizmor/strict.txt
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=69
aspect_field has_subject=True
aspect_field subject_label=//quality:strict_generated_policy_under_test
aspect_field transitive_count=0"""

def policy_profile_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":generated_policy_under_test",
            ":merged_generated_policy_under_test",
            ":strict_generated_policy_under_test",
        ],
        expected_observations = EXPECTED_PROFILE_OBSERVATIONS,
    )
