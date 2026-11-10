# Quality test configuration design

This page fixes how tests under `core/quality` choose build settings. The test macros, the case tables and the suites in `core/quality` and `core/quality/testdata` follow it.

## Unit of cost

- The unit is the configuration: one distinct set of build setting values below a test.
- A test adds a configuration when it changes any setting, not only `//config:workspace`. `//config:config_profile`, `//cc:compiler`, the `//config:tool_*` flags, the rules_rust lint flags, `platforms` and `extra_execution_platforms` all count.
- Two tests share a configuration only when every setting has the same value and both reach their subject the same way.
- A policy target that no test selects costs nothing. Policy targets that are the direct subject of a provider test are outside this page.

## Configuration table

- `core/quality/testdata/test_configs.bzl` holds `TEST_CONFIGS`, a dict from a configuration name to its settings.
- Every test under `core/quality` that changes a build setting names one entry of `TEST_CONFIGS`. No other file writes a settings dict for a quality test.
- An entry lists every setting the table controls. A setting a test does not care about takes the table default.
- The table file holds `MAX_TEST_CONFIGS`. A unit test fails when the table has more entries than the limit, when two entries have equal settings, and when an entry has no test.
- A change that raises `MAX_TEST_CONFIGS` states the measured cost of the new entry.

## One transition

- `core/quality/testdata/config_subject.bzl` defines the only transition for quality tests. It takes a configuration name and writes that entry's settings.
- The transition has no inputs. It never reads the incoming value of a setting, so a test has the same configuration under every command line.
- The consumer policy tests, the real result tests and the per-fixture result rules use this transition. No package declares its own transition or its own `_results` rule.
- Per-test attributes such as `policy`, `compiler`, `config_profile`, `clippy_flags`, `tool_govet` and `tool_dialyzer` do not exist. A case names a configuration.

## Analysis tests

- An analysis test under `core/quality` does not pass `config_settings`. It points `target_under_test` at a subject that applies the transition, and reads providers the subject forwards.
- A test that expects an analysis failure uses `expect_failure` on such a subject. Failure tests of one package share one entry where their subjects allow it.
- Settings that are not quality settings, such as coverage flags and `pipelined_compilation`, stay in `config_settings` of the test that proves them. They are not combined with a quality setting.

## Policies

- `shared` is the default entry. Its policy binds every family and every tool whose settings do not conflict. A case uses `shared` unless a rule below gives it another entry.
- A policy does not exist per tool, per capability or per fixture directory. `<family>_owned` policies are entries only when a rule below requires them.
- A case gets a variant policy only when its assertion depends on the policy: a family, tool, capability or config is absent or disabled, or two cases need different settings for the same family.
- A variant policy extends the shared policy and changes only what the case asserts.
- The generated policy and the default workspace policy are one entry each.

## Tiers

- A tier is selected by `//config:config_profile`, never by a second policy.
- The shared policy binds tier configs that select on the profile. `standard` and `strict` cases of every tool share two entries: `shared` and `shared_standard`.
- No policy name contains `standard` or `strict`.

## Tool overrides

- All tool override proofs share one entry that sets every `//config:tool_*` flag under test at once.
- The matching default proofs use `shared` and set no flag.
- A proof that needs a wrong, empty or duplicate tool binding has its own entry. Each such entry changes one flag.

## Compilers and lint flags

- `//cc:compiler` has one entry per compiler that has a case.
- The rules_rust lint flags have one entry per tier. They are part of `shared` and `shared_standard`.
- Output-diagnostics flags have the same value in every entry.

## Platforms

- An entry that sets `platforms` or `extra_execution_platforms` forks the exec configuration. A change that adds one states the target cost and the exec cost.
- Each platform has at most one entry per policy. Platform values are the platform labels in `//qt`.
- A test that only needs a result to be marked for another platform uses `platforms` alone.

## Aspects

- A subject rule attaches only the aspects of the family and capability its case asserts.
- No rule attaches a list of every family aspect.
- Subject rules live in `core/quality/testdata/real_aspect_subject.bzl`, keyed by file class.

## Suites and wildcards

- No target is named `all`. `:all` is always the package wildcard.
- Every test in `core/quality` and `core/quality/testdata` belongs to exactly one named `test_suite` of its package. A unit test enforces it.
- A test is tagged `manual` only when a `manual` suite lists it and `CONTRIBUTING.md` names that suite.
- A suite groups tests that share entries, so one suite can be analyzed alone.

## Measuring

- Cost is measured per suite on a fresh Bazel server:

```sh
bazel shutdown
bazel build --nobuild --keep_going //core/quality/testdata:<suite>
bazel config --output=json --dump_all
```

- The first number is `targets configured` from the `Analyzed` line. The second is the count of configurations in the dump, split by `isExec`.
- The cost of one entry is the `targets configured` of a test that uses it, run after a test of `shared` in the same server.
- A change to `TEST_CONFIGS`, to the transition or to a subject rule reports both numbers before and after for each suite it touches.
