"""Unit and analysis tests for workspace policy and applicability."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":applicability.bzl", "capability_selection", "effective_classes", "selected_adapters")
load(":policy.bzl", "merge_configs", "merge_policy")
load("//core/quality/private:generated_policy.bzl", "generated_families_error")

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

EXPECTED_POLICY_OBSERVATIONS = """subject //core/quality:disable_policy_under_test
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
aspect_field subject_label=//core/quality:disable_policy_under_test
aspect_field transitive_count=0
subject //core/quality:extends_policy_under_test
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
aspect_field subject_label=//core/quality:extends_policy_under_test
aspect_field transitive_count=0
subject //core/quality:override_policy_under_test
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
aspect_field subject_label=//core/quality:override_policy_under_test
aspect_field transitive_count=0
subject //core/quality:policy_under_test
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
aspect_field subject_label=//core/quality:policy_under_test
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

EXPECTED_PROFILE_OBSERVATIONS = """subject //core/quality:generated_policy_under_test
field config.ansible_lint=core/quality/tiers/ansible_lint/standard.yaml
field config.bicep=core/quality/tiers/bicep/standard.bicepconfig.json
field config.biome=core/quality/tiers/biome/standard.json
field config.buf=core/quality/tiers/buf/standard.yaml
field config.buildifier=core/quality/tiers/buildifier/standard.json
field config.cfn_lint=core/quality/tiers/cfn_lint/standard.yaml
field config.checkmake=core/quality/tiers/checkmake/standard.ini
field config.clang_format=core/quality/tiers/clang_format/standard.clang-format
field config.clang_tidy=core/quality/tiers/clang_tidy/standard.clang-tidy
field config.cmakelint=core/quality/tiers/cmakelint/standard.cmakelintrc
field config.credo=core/quality/tiers/credo/standard.exs
field config.dart=core/quality/tiers/dart/standard.yaml
field config.detekt=core/quality/tiers/detekt/standard.yml
field config.docstrfmt=core/quality/tiers/docstrfmt/standard.toml
field config.elvis=core/quality/tiers/elvis/standard.config
field config.errcheck=core/quality/tiers/errcheck/standard.txt
field config.eslint=core/quality/tiers/eslint/standard.mjs
field config.ffmt=core/quality/tiers/ffmt/standard.toml
field config.fortitude=core/quality/tiers/fortitude/standard.toml
field config.fsharplint=core/quality/tiers/fsharplint/standard.json
field config.gersemi=core/quality/tiers/gersemi/standard.yaml
field config.govet=core/quality/tiers/govet/standard.json
field config.gplint=core/quality/tiers/gplint/standard.json
field config.graphql_eslint=core/quality/tiers/graphql_eslint/standard.json
field config.hadolint=core/quality/tiers/hadolint/standard.yaml
field config.hlint=core/quality/tiers/hlint/standard.yaml
field config.luals=core/quality/tiers/luals/standard.json
field config.lychee=core/quality/tiers/lychee/standard.toml
field config.markdown_check=core/quality/tiers/markdown_check/standard.txt
field config.mbake=core/quality/tiers/mbake/standard.toml
field config.mix_format=core/quality/tiers/mix_format/standard.exs
field config.perlcritic=core/quality/tiers/perlcritic/standard.perlcriticrc
field config.perltidy=core/quality/tiers/perltidy/standard.perltidyrc
field config.phpcs=core/quality/tiers/phpcs/standard.xml
field config.phpstan=core/quality/tiers/phpstan/standard.neon
field config.pmd=core/quality/tiers/pmd/standard.xml
field config.psscriptanalyzer=core/quality/tiers/psscriptanalyzer/standard.psd1
field config.pydoclint=core/quality/tiers/pydoclint/standard.toml
field config.qmlformat=core/quality/tiers/qmlformat/standard.qmlformat.ini
field config.qmllint=core/quality/tiers/qmllint/standard.qmllint.ini
field config.redocly=core/quality/tiers/redocly/standard.yaml
field config.regal=core/quality/tiers/regal/standard.yaml
field config.roslyn=core/quality/tiers/roslyn/standard.globalconfig
field config.rubocop=core/quality/tiers/rubocop/standard.yml
field config.ruff=core/quality/tiers/ruff/strict.toml
field config.rustfmt=core/quality/tiers/rustfmt/standard.toml
field config.scalafix=core/quality/tiers/scalafix/standard.conf
field config.scalafmt=core/quality/tiers/scalafmt/standard.conf
field config.selene=core/quality/tiers/selene/standard.toml
field config.shellcheck=core/quality/tiers/shellcheck/standard.shellcheckrc
field config.shfmt=core/quality/tiers/shfmt/standard.editorconfig
field config.spotbugs=core/quality/tiers/spotbugs/standard.xml
field config.sqlfluff=core/quality/tiers/sqlfluff/standard.cfg
field config.staticcheck=core/quality/tiers/staticcheck/standard.conf
field config.stylelint=core/quality/tiers/stylelint/standard.json
field config.stylua=core/quality/tiers/stylua/standard.toml
field config.swift_format=core/quality/tiers/swift_format/standard.json
field config.swift_lint=core/quality/tiers/swift_lint/standard.json
field config.taplo=core/quality/tiers/taplo/standard.toml
field config.tflint=core/quality/tiers/tflint/standard.hcl
field config.trivy_config=core/quality/tiers/trivy_config/standard.txt
field config.ty=core/quality/tiers/ty/standard.toml
field config.typos=core/quality/tiers/typos/standard.toml
field config.vale=core/quality/corpus_vale.ini
field config.yamlfmt=core/quality/tiers/yamlfmt/standard.yaml
field config.yamllint=core/quality/tiers/yamllint/standard.yaml
field config.zizmor=core/quality/tiers/zizmor/standard.txt
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
aspect_field field_count=76
aspect_field has_subject=True
aspect_field subject_label=//core/quality:generated_policy_under_test
aspect_field transitive_count=0
subject //core/quality:merged_generated_policy_under_test
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
aspect_field subject_label=//core/quality:merged_generated_policy_under_test
aspect_field transitive_count=0
subject //core/quality:strict_generated_policy_under_test
field config.ansible_lint=core/quality/tiers/ansible_lint/strict.yaml
field config.bicep=core/quality/tiers/bicep/strict.bicepconfig.json
field config.biome=core/quality/tiers/biome/strict.json
field config.buf=core/quality/tiers/buf/strict.yaml
field config.buildifier=core/quality/tiers/buildifier/strict.json
field config.cfn_lint=core/quality/tiers/cfn_lint/strict.yaml
field config.checkmake=core/quality/tiers/checkmake/strict.ini
field config.clang_format=core/quality/tiers/clang_format/strict.clang-format
field config.clang_tidy=core/quality/tiers/clang_tidy/strict.clang-tidy
field config.cmakelint=core/quality/tiers/cmakelint/strict.cmakelintrc
field config.credo=core/quality/tiers/credo/strict.exs
field config.dart=core/quality/tiers/dart/strict.yaml
field config.detekt=core/quality/tiers/detekt/strict.yml
field config.docstrfmt=core/quality/tiers/docstrfmt/strict.toml
field config.elvis=core/quality/tiers/elvis/strict.config
field config.errcheck=core/quality/tiers/errcheck/strict.txt
field config.eslint=core/quality/tiers/eslint/strict.mjs
field config.ffmt=core/quality/tiers/ffmt/strict.toml
field config.fortitude=core/quality/tiers/fortitude/strict.toml
field config.fsharplint=core/quality/tiers/fsharplint/strict.json
field config.gersemi=core/quality/tiers/gersemi/strict.yaml
field config.govet=core/quality/tiers/govet/strict.json
field config.gplint=core/quality/tiers/gplint/strict.json
field config.graphql_eslint=core/quality/tiers/graphql_eslint/strict.json
field config.hadolint=core/quality/tiers/hadolint/strict.yaml
field config.hlint=core/quality/tiers/hlint/strict.yaml
field config.luals=core/quality/tiers/luals/strict.json
field config.lychee=core/quality/tiers/lychee/strict.toml
field config.markdown_check=core/quality/tiers/markdown_check/strict.txt
field config.mbake=core/quality/tiers/mbake/strict.toml
field config.mix_format=core/quality/tiers/mix_format/strict.exs
field config.perlcritic=core/quality/tiers/perlcritic/strict.perlcriticrc
field config.perltidy=core/quality/tiers/perltidy/strict.perltidyrc
field config.phpcs=core/quality/tiers/phpcs/strict.xml
field config.phpstan=core/quality/tiers/phpstan/strict.neon
field config.pmd=core/quality/tiers/pmd/strict.xml
field config.psscriptanalyzer=core/quality/tiers/psscriptanalyzer/strict.psd1
field config.pydoclint=core/quality/tiers/pydoclint/strict.toml
field config.qmlformat=core/quality/tiers/qmlformat/strict.qmlformat.ini
field config.qmllint=core/quality/tiers/qmllint/strict.qmllint.ini
field config.redocly=core/quality/tiers/redocly/strict.yaml
field config.regal=core/quality/tiers/regal/strict.yaml
field config.roslyn=core/quality/tiers/roslyn/strict.globalconfig
field config.rubocop=core/quality/tiers/rubocop/strict.yml
field config.ruff=core/quality/tiers/ruff/strict.toml
field config.rustfmt=core/quality/tiers/rustfmt/strict.toml
field config.scalafix=core/quality/tiers/scalafix/strict.conf
field config.scalafmt=core/quality/tiers/scalafmt/strict.conf
field config.selene=core/quality/tiers/selene/strict.toml
field config.shellcheck=core/quality/tiers/shellcheck/strict.shellcheckrc
field config.shfmt=core/quality/tiers/shfmt/strict.editorconfig
field config.spotbugs=core/quality/tiers/spotbugs/strict.xml
field config.sqlfluff=core/quality/tiers/sqlfluff/strict.cfg
field config.staticcheck=core/quality/tiers/staticcheck/strict.conf
field config.stylelint=core/quality/tiers/stylelint/strict.json
field config.stylua=core/quality/tiers/stylua/strict.toml
field config.swift_format=core/quality/tiers/swift_format/strict.json
field config.swift_lint=core/quality/tiers/swift_lint/strict.json
field config.taplo=core/quality/tiers/taplo/strict.toml
field config.tflint=core/quality/tiers/tflint/strict.hcl
field config.trivy_config=core/quality/tiers/trivy_config/strict.txt
field config.ty=core/quality/tiers/ty/strict.toml
field config.typos=core/quality/tiers/typos/strict.toml
field config.vale=core/quality/tiers/vale/strict.ini
field config.yamlfmt=core/quality/tiers/yamlfmt/strict.yaml
field config.yamllint=core/quality/tiers/yamllint/strict.yaml
field config.zizmor=core/quality/tiers/zizmor/strict.txt
field disabled.python.audit=False
field disabled.python.format=False
field disabled.python.lint=False
field disabled.python.typecheck=False
field family.python.audit=
field family.python.format=
field family.python.lint=lint-a
field family.python.typecheck=
aspect_field aspect_seen=True
aspect_field field_count=75
aspect_field has_subject=True
aspect_field subject_label=//core/quality:strict_generated_policy_under_test
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
