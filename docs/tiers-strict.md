# Strict tiers

This page shows what the strict tier changes against the standard tier of each tool. A tier file is self-contained, so the diff is the whole difference. This page is generated. To refresh it, run `bazel run //docs:update_tiers`.

## buf

Source: dx. Native file: `buf.yaml`.

```diff
--- a/core/quality/tiers/buf/strict.yaml
+++ b/core/quality/tiers/buf/strict.yaml
@@ -2,6 +2,8 @@
 lint:
   use:
     - STANDARD
+    - COMMENTS
+    - UNARY_RPC
 breaking:
   use:
-    - WIRE_JSON
+    - FILE
```

## buildifier

Source: dx. Native file: `.buildifier.json`.

```diff
--- a/core/quality/tiers/buildifier/strict.json
+++ b/core/quality/tiers/buildifier/strict.json
@@ -1,3 +1,3 @@
 {
-  "warnings": "default"
+  "warnings": "all"
 }
```

## checkmake

Source: dx. Native file: `checkmake.ini`.

```diff
--- a/core/quality/tiers/checkmake/strict.ini
+++ b/core/quality/tiers/checkmake/strict.ini
@@ -1,5 +1,6 @@
 [maxbodylength]
-disabled=true
+disabled=false
+maxBodyLength=5
 
 [minphony]
 disabled=false
```

## cmakelint

Source: dx. Native file: `.cmakelintrc`.

```diff
--- a/core/quality/tiers/cmakelint/strict.cmakelintrc
+++ b/core/quality/tiers/cmakelint/strict.cmakelintrc
@@ -1,2 +1,2 @@
 # gersemi owns layout, so the whitespace and line length checks are off.
-filter=-linelength,-whitespace/eol,-whitespace/extra,-whitespace/indent,-whitespace/mismatch,-whitespace/newline,-whitespace/tabs,-convention/filename
+filter=-linelength,-whitespace/eol,-whitespace/extra,-whitespace/indent,-whitespace/mismatch,-whitespace/newline,-whitespace/tabs
```

## dart

Source: dx. Native file: `analysis_options.yaml`.

```diff
--- a/core/quality/tiers/dart/strict.yaml
+++ b/core/quality/tiers/dart/strict.yaml
@@ -93,3 +93,29 @@
     - use_super_parameters
     - valid_regexps
     - void_checks
+    - avoid_dynamic_calls
+    - avoid_returning_null_for_future
+    - avoid_type_to_string
+    - avoid_void_async
+    - cancel_subscriptions
+    - cast_nullable_to_non_nullable
+    - close_sinks
+    - discarded_futures
+    - invalid_case_patterns
+    - literal_only_boolean_expressions
+    - no_adjacent_strings_in_list
+    - no_runtimeType_toString
+    - noop_primitive_operations
+    - only_throw_errors
+    - prefer_void_to_null
+    - test_types_in_equals
+    - throw_in_finally
+    - unawaited_futures
+    - unnecessary_await_in_return
+    - unnecessary_breaks
+    - unnecessary_statements
+analyzer:
+  language:
+    strict-casts: true
+    strict-inference: true
+    strict-raw-types: true
```

## docstrfmt

Source: dx. Native file: `docstrfmt.toml`.

```diff
--- a/core/quality/tiers/docstrfmt/strict.toml
+++ b/core/quality/tiers/docstrfmt/strict.toml
@@ -2,4 +2,5 @@
 bullet-list-marker = "-"
 format-python-code-blocks = true
 indent-width = 3
-line-length = 88
+line-length = 80
+ordered-marker = "1"
```

## eslint

Source: dx. Native file: `eslint.config.mjs, eslint.config.js, eslint.config.cjs, eslint.config.ts, .eslintrc*`.

```diff
--- a/core/quality/tiers/eslint/strict.mjs
+++ b/core/quality/tiers/eslint/strict.mjs
@@ -1,7 +1,7 @@
 import { defineConfig } from "eslint/config";
 import globals from "globals";
 import tseslint from "typescript-eslint";
-import frameworks from "./frameworks.mjs";
+import frameworks, { strict as frameworkRules } from "./frameworks.mjs";
 
 const typed = process.env.DX_ESLINT_TYPED === "1";
 const typescriptFiles = ["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.cts"];
@@ -98,33 +98,56 @@
 	"prefer-rest-params": "error",
 	"prefer-spread": "error",
 	"@typescript-eslint/await-thenable": "error",
-	"@typescript-eslint/ban-ts-comment": "error",
+	"@typescript-eslint/ban-ts-comment": [
+		"error",
+		{
+			minimumDescriptionLength: 10,
+		},
+	],
 	"no-array-constructor": "off",
 	"@typescript-eslint/no-array-constructor": "error",
 	"@typescript-eslint/no-array-delete": "error",
 	"@typescript-eslint/no-base-to-string": "error",
+	"@typescript-eslint/no-confusing-void-expression": "error",
+	"@typescript-eslint/no-deprecated": "error",
 	"@typescript-eslint/no-duplicate-enum-values": "error",
 	"@typescript-eslint/no-duplicate-type-constituents": "error",
+	"@typescript-eslint/no-dynamic-delete": "error",
 	"@typescript-eslint/no-empty-object-type": "error",
 	"@typescript-eslint/no-explicit-any": "error",
 	"@typescript-eslint/no-extra-non-null-assertion": "error",
+	"@typescript-eslint/no-extraneous-class": "error",
 	"@typescript-eslint/no-floating-promises": "error",
 	"@typescript-eslint/no-for-in-array": "error",
+	"@typescript-eslint/no-generated-empty-object-type": "error",
 	"no-implied-eval": "off",
 	"@typescript-eslint/no-implied-eval": "error",
+	"@typescript-eslint/no-invalid-void-type": "error",
+	"@typescript-eslint/no-meaningless-void-operator": "error",
 	"@typescript-eslint/no-misused-new": "error",
 	"@typescript-eslint/no-misused-promises": "error",
+	"@typescript-eslint/no-misused-spread": "error",
+	"@typescript-eslint/no-mixed-enums": "error",
 	"@typescript-eslint/no-namespace": "error",
+	"@typescript-eslint/no-non-null-asserted-nullish-coalescing": "error",
 	"@typescript-eslint/no-non-null-asserted-optional-chain": "error",
+	"@typescript-eslint/no-non-null-assertion": "error",
 	"@typescript-eslint/no-redundant-type-constituents": "error",
 	"@typescript-eslint/no-require-imports": "error",
 	"@typescript-eslint/no-this-alias": "error",
+	"@typescript-eslint/no-unnecessary-boolean-literal-compare": "error",
+	"@typescript-eslint/no-unnecessary-condition": "error",
+	"@typescript-eslint/no-unnecessary-template-expression": "error",
+	"@typescript-eslint/no-unnecessary-type-arguments": "error",
 	"@typescript-eslint/no-unnecessary-type-assertion": "error",
 	"@typescript-eslint/no-unnecessary-type-constraint": "error",
+	"@typescript-eslint/no-unnecessary-type-conversion": "error",
+	"@typescript-eslint/no-unnecessary-type-parameters": "error",
 	"@typescript-eslint/no-unsafe-argument": "error",
 	"@typescript-eslint/no-unsafe-assignment": "error",
 	"@typescript-eslint/no-unsafe-call": "error",
 	"@typescript-eslint/no-unsafe-declaration-merging": "error",
+	"@typescript-eslint/no-unsafe-enum-assignment": "error",
 	"@typescript-eslint/no-unsafe-enum-comparison": "error",
 	"@typescript-eslint/no-unsafe-function-type": "error",
 	"@typescript-eslint/no-unsafe-member-access": "error",
@@ -134,19 +157,52 @@
 	"@typescript-eslint/no-unused-expressions": "error",
 	"no-unused-vars": "off",
 	"@typescript-eslint/no-unused-vars": "error",
+	"no-useless-constructor": "off",
+	"@typescript-eslint/no-useless-constructor": "error",
+	"@typescript-eslint/no-useless-default-assignment": "error",
 	"@typescript-eslint/no-wrapper-object-types": "error",
 	"no-throw-literal": "off",
 	"@typescript-eslint/only-throw-error": "error",
 	"@typescript-eslint/prefer-as-const": "error",
+	"@typescript-eslint/prefer-literal-enum-member": "error",
 	"@typescript-eslint/prefer-namespace-keyword": "error",
 	"prefer-promise-reject-errors": "off",
 	"@typescript-eslint/prefer-promise-reject-errors": "error",
+	"@typescript-eslint/prefer-reduce-type-parameter": "error",
+	"@typescript-eslint/prefer-return-this-type": "error",
+	"@typescript-eslint/related-getter-setter-pairs": "error",
 	"require-await": "off",
 	"@typescript-eslint/require-await": "error",
-	"@typescript-eslint/restrict-plus-operands": "error",
-	"@typescript-eslint/restrict-template-expressions": "error",
+	"@typescript-eslint/restrict-plus-operands": [
+		"error",
+		{
+			allowAny: false,
+			allowBoolean: false,
+			allowNullish: false,
+			allowNumberAndString: false,
+			allowRegExp: false,
+		},
+	],
+	"@typescript-eslint/restrict-template-expressions": [
+		"error",
+		{
+			allowAny: false,
+			allowBoolean: false,
+			allowNever: false,
+			allowNullish: false,
+			allowNumber: false,
+			allowRegExp: false,
+		},
+	],
+	"no-return-await": "off",
+	"@typescript-eslint/return-await": [
+		"error",
+		"error-handling-correctness-only",
+	],
 	"@typescript-eslint/triple-slash-reference": "error",
 	"@typescript-eslint/unbound-method": "error",
+	"@typescript-eslint/unified-signatures": "error",
+	"@typescript-eslint/use-unknown-in-catch-callback-variable": "error",
 };
 
 export default defineConfig(
@@ -172,4 +228,6 @@
 			: {},
 	},
 	frameworks,
+	frameworkRules,
+	{ linterOptions: { reportUnusedDisableDirectives: "error" } },
 );
```

## gersemi

Source: dx. Native file: `.gersemirc`.

```diff
--- a/core/quality/tiers/gersemi/strict.yaml
+++ b/core/quality/tiers/gersemi/strict.yaml
@@ -2,5 +2,5 @@
 line_length: 80
 list_expansion: favour-inlining
 sort_order: case-sensitive
-unsafe: true
-warn_about_unknown_commands: false
+unsafe: false
+warn_about_unknown_commands: true
```

## hlint

Source: dx. Native file: `.hlint.yaml`.

```diff
--- a/core/quality/tiers/hlint/strict.yaml
+++ b/core/quality/tiers/hlint/strict.yaml
@@ -1,6 +1,3 @@
-- ignore: {name: Use camelCase}
-- ignore: {name: Use section}
-- ignore: {name: Use lambda-case}
-- ignore: {name: Use tuple-section}
-- ignore: {name: Redundant bracket}
-- ignore: {name: Move brackets to avoid $}
+- group: {name: extra, enabled: true}
+- group: {name: generalise, enabled: true}
+- group: {name: future, enabled: true}
```

## lychee

Source: dx. Native file: `lychee.toml`.

```diff
--- a/core/quality/tiers/lychee/strict.toml
+++ b/core/quality/tiers/lychee/strict.toml
@@ -1,4 +1,4 @@
-# Link checks use lychee's defaults, written down so no other config is read.
-timeout = 20
-max_retries = 3
-max_redirects = 10
+# Fail sooner on slow or redirecting links.
+timeout = 10
+max_retries = 1
+max_redirects = 5
```

## mbake

Source: dx. Native file: `.bake.toml`.

```diff
--- a/core/quality/tiers/mbake/strict.toml
+++ b/core/quality/tiers/mbake/strict.toml
@@ -1,15 +1,15 @@
 [formatter]
 align_variable_assignments = false
-auto_insert_phony_declarations = false
+auto_insert_phony_declarations = true
 ensure_final_newline = true
 fix_missing_recipe_tabs = true
-group_phony_declarations = false
-indent_nested_conditionals = false
+group_phony_declarations = true
+indent_nested_conditionals = true
 max_consecutive_empty_lines = 2
 max_line_length = 120
 normalize_empty_lines = true
 normalize_line_continuations = true
-phony_at_top = false
+phony_at_top = true
 remove_trailing_whitespace = true
 space_after_colon = true
 space_around_assignment = true
```

## mix_format

Source: dx. Native file: `.formatter.exs`.

```diff
--- a/core/quality/tiers/mix_format/strict.exs
+++ b/core/quality/tiers/mix_format/strict.exs
@@ -1,4 +1,4 @@
-# mix format defaults, written down so no other .formatter.exs is read.
+# Shorter lines.
 [
-  line_length: 98
+  line_length: 80
 ]
```

## perltidy

Source: dx. Native file: `.perltidyrc`.

```diff
--- a/core/quality/tiers/perltidy/strict.perltidyrc
+++ b/core/quality/tiers/perltidy/strict.perltidyrc
@@ -1,3 +1,2 @@
-# perltidy defaults, written down so no other profile is read.
---indent-columns=4
---maximum-line-length=80
+# Perl Best Practices layout.
+--perl-best-practices
```

## psscriptanalyzer

Source: dx. Native file: `PSScriptAnalyzerSettings.psd1`.

```diff
--- a/core/quality/tiers/psscriptanalyzer/strict.psd1
+++ b/core/quality/tiers/psscriptanalyzer/strict.psd1
@@ -1,6 +1,8 @@
 @{
     IncludeDefaultRules = $true
     Rules               = @{
+        PSAvoidExclaimOperator     = @{ Enable = $true }
+        PSAvoidUsingWriteHost      = @{ Enable = $true }
         PSUseConsistentIndentation = @{ Enable = $true }
     }
 }
```

## pydoclint

Source: dx. Native file: `*.toml`.

```diff
--- a/core/quality/tiers/pydoclint/strict.toml
+++ b/core/quality/tiers/pydoclint/strict.toml
@@ -1,5 +1,11 @@
-# Numpy-style docstrings. Short docstrings are not checked.
+# Numpy-style docstrings. Every docstring is checked, including class attributes and yields.
 [tool.pydoclint]
 style = "numpy"
-skip-checking-short-docstrings = true
+skip-checking-short-docstrings = false
 check-arg-order = true
+check-class-attributes = true
+check-yield-types = true
+check-style-mismatch = true
+check-arg-defaults = true
+require-return-section-when-returning-nothing = true
+require-yield-section-when-yielding-nothing = true
```

## ruff

Source: dx. Native file: `ruff.toml, .ruff.toml`.

```diff
--- a/core/quality/tiers/ruff/strict.toml
+++ b/core/quality/tiers/ruff/strict.toml
@@ -1,94 +1,63 @@
-# Syntax errors, pyflakes, bugbear, security, deprecated APIs, performance and unused code.
-# No style rules. Codes come from the pinned `ruff rule --all --output-format json`.
+# Every stable ruff rule except the ones named below. Rebuild the list with
+#   ruff rule --all --output-format json
+# (the pinned ruff), keeping rules whose status is Stable and not preview, minus:
+#   formatter conflicts: W191 E111 E114 E117 E501 D206 D300 Q000-Q004 COM812 COM819 ISC001 ISC002
+#   contradicting D pairs, keeping D211, D212 and D406: D203 D213 D416
+#   project inputs: the copyright header rule and INP001
 [lint]
 select = [
-  # syntax errors
-  "E902",
-  # pyflakes
-  "F401",
-  "F402",
-  "F403",
-  "F404",
-  "F405",
-  "F406",
-  "F407",
-  "F501",
-  "F502",
-  "F503",
-  "F504",
-  "F505",
-  "F506",
-  "F507",
-  "F508",
-  "F509",
-  "F521",
-  "F522",
-  "F523",
-  "F524",
-  "F525",
-  "F541",
-  "F601",
-  "F602",
-  "F621",
-  "F622",
-  "F631",
-  "F632",
-  "F633",
-  "F634",
-  "F701",
-  "F702",
-  "F704",
-  "F706",
-  "F707",
-  "F722",
-  "F811",
-  "F821",
-  "F822",
-  "F823",
-  "F841",
-  "F842",
-  "F901",
-  # bugbear
-  "B002",
-  "B003",
-  "B004",
-  "B005",
-  "B006",
-  "B007",
-  "B008",
-  "B009",
-  "B010",
-  "B011",
-  "B012",
-  "B013",
-  "B014",
-  "B015",
-  "B016",
-  "B017",
-  "B018",
-  "B019",
-  "B020",
-  "B021",
-  "B022",
-  "B023",
-  "B024",
-  "B025",
-  "B026",
-  "B027",
-  "B028",
-  "B029",
-  "B030",
-  "B031",
-  "B032",
-  "B033",
-  "B034",
-  "B035",
-  "B039",
-  "B904",
-  "B905",
-  "B911",
-  "B912",
-  # security
+  # Airflow
+  "AIR001",
+  "AIR002",
+  "AIR301",
+  "AIR302",
+  "AIR303",
+  "AIR311",
+  "AIR312",
+  # eradicate
+  "ERA001",
+  # FastAPI
+  "FAST001",
+  "FAST002",
+  "FAST003",
+  # flake8-2020
+  "YTT101",
+  "YTT102",
+  "YTT103",
+  "YTT201",
+  "YTT202",
+  "YTT203",
+  "YTT204",
+  "YTT301",
+  "YTT302",
+  "YTT303",
+  # flake8-annotations
+  "ANN001",
+  "ANN002",
+  "ANN003",
+  "ANN201",
+  "ANN202",
+  "ANN204",
+  "ANN205",
+  "ANN206",
+  "ANN401",
+  # flake8-async
+  "ASYNC100",
+  "ASYNC105",
+  "ASYNC109",
+  "ASYNC110",
+  "ASYNC115",
+  "ASYNC116",
+  "ASYNC210",
+  "ASYNC212",
+  "ASYNC220",
+  "ASYNC221",
+  "ASYNC222",
+  "ASYNC230",
+  "ASYNC240",
+  "ASYNC250",
+  "ASYNC251",
+  # flake8-bandit
   "S101",
   "S102",
   "S103",
@@ -147,29 +116,748 @@
   "S701",
   "S702",
   "S704",
-  # deprecated APIs
-  "UP005",
-  "UP019",
-  "UP026",
-  "UP035",
-  "UP036",
-  # performance
-  "PERF101",
-  "PERF102",
-  "PERF203",
-  "PERF401",
-  "PERF402",
-  "PERF403",
-  # unused
-  "ARG001",
-  "ARG002",
-  "ARG003",
-  "ARG004",
-  "ARG005",
-  "RUF059",
-]
-preview = false
-
-[lint.per-file-ignores]
-"*_test.py" = ["S101", "ARG"]
-"test_*.py" = ["S101", "ARG"]
+  # flake8-blind-except
+  "BLE001",
+  # flake8-boolean-trap
+  "FBT001",
+  "FBT002",
+  "FBT003",
+  # flake8-bugbear
+  "B002",
+  "B003",
+  "B004",
+  "B005",
+  "B006",
+  "B007",
+  "B008",
+  "B009",
+  "B010",
+  "B011",
+  "B012",
+  "B013",
+  "B014",
+  "B015",
+  "B016",
+  "B017",
+  "B018",
+  "B019",
+  "B020",
+  "B021",
+  "B022",
+  "B023",
+  "B024",
+  "B025",
+  "B026",
+  "B027",
+  "B028",
+  "B029",
+  "B030",
+  "B031",
+  "B032",
+  "B033",
+  "B034",
+  "B035",
+  "B039",
+  "B904",
+  "B905",
+  "B911",
+  "B912",
+  # flake8-builtins
+  "A001",
+  "A002",
+  "A003",
+  "A004",
+  "A005",
+  "A006",
+  # flake8-commas
+  "COM818",
+  # flake8-comprehensions
+  "C400",
+  "C401",
+  "C402",
+  "C403",
+  "C404",
+  "C405",
+  "C406",
+  "C408",
+  "C409",
+  "C410",
+  "C411",
+  "C413",
+  "C414",
+  "C415",
+  "C416",
+  "C417",
+  "C418",
+  "C419",
+  "C420",
+  # flake8-datetimez
+  "DTZ001",
+  "DTZ002",
+  "DTZ003",
+  "DTZ004",
+  "DTZ005",
+  "DTZ006",
+  "DTZ007",
+  "DTZ011",
+  "DTZ012",
+  "DTZ901",
+  # flake8-debugger
+  "T100",
+  # flake8-django
+  "DJ001",
+  "DJ003",
+  "DJ006",
+  "DJ007",
+  "DJ008",
+  "DJ012",
+  "DJ013",
+  # flake8-errmsg
+  "EM101",
+  "EM102",
+  "EM103",
+  # flake8-executable
+  "EXE001",
+  "EXE002",
+  "EXE003",
+  "EXE004",
+  "EXE005",
+  # flake8-fixme
+  "FIX001",
+  "FIX002",
+  "FIX003",
+  "FIX004",
+  # flake8-future-annotations
+  "FA100",
+  "FA102",
+  # flake8-gettext
+  "INT001",
+  "INT002",
+  "INT003",
+  # flake8-implicit-str-concat
+  "ISC003",
+  "ISC004",
+  # flake8-import-conventions
+  "ICN001",
+  "ICN002",
+  "ICN003",
+  # flake8-logging
+  "LOG001",
+  "LOG002",
+  "LOG004",
+  "LOG007",
+  "LOG009",
+  "LOG014",
+  "LOG015",
+  # flake8-logging-format
+  "G001",
+  "G002",
+  "G003",
+  "G004",
+  "G010",
+  "G101",
+  "G201",
+  "G202",
+  # flake8-pie
+  "PIE790",
+  "PIE794",
+  "PIE796",
+  "PIE800",
+  "PIE804",
+  "PIE807",
+  "PIE808",
+  "PIE810",
+  # flake8-print
+  "T201",
+  "T203",
+  # flake8-pyi
+  "PYI001",
+  "PYI002",
+  "PYI003",
+  "PYI004",
+  "PYI005",
+  "PYI006",
+  "PYI007",
+  "PYI008",
+  "PYI009",
+  "PYI010",
+  "PYI011",
+  "PYI012",
+  "PYI013",
+  "PYI014",
+  "PYI015",
+  "PYI016",
+  "PYI017",
+  "PYI018",
+  "PYI019",
+  "PYI020",
+  "PYI021",
+  "PYI024",
+  "PYI025",
+  "PYI026",
+  "PYI029",
+  "PYI030",
+  "PYI032",
+  "PYI033",
+  "PYI034",
+  "PYI035",
+  "PYI036",
+  "PYI041",
+  "PYI042",
+  "PYI043",
+  "PYI044",
+  "PYI045",
+  "PYI046",
+  "PYI047",
+  "PYI048",
+  "PYI049",
+  "PYI050",
+  "PYI051",
+  "PYI052",
+  "PYI053",
+  "PYI054",
+  "PYI055",
+  "PYI056",
+  "PYI057",
+  "PYI058",
+  "PYI059",
+  "PYI061",
+  "PYI062",
+  "PYI063",
+  "PYI064",
+  "PYI066",
+  # flake8-pytest-style
+  "PT001",
+  "PT002",
+  "PT003",
+  "PT006",
+  "PT007",
+  "PT008",
+  "PT009",
+  "PT010",
+  "PT011",
+  "PT012",
+  "PT013",
+  "PT014",
+  "PT015",
+  "PT016",
+  "PT017",
+  "PT018",
+  "PT019",
+  "PT020",
+  "PT021",
+  "PT022",
+  "PT023",
+  "PT024",
+  "PT025",
+  "PT026",
+  "PT027",
+  "PT028",
+  "PT030",
+  "PT031",
+  # flake8-raise
+  "RSE102",
+  # flake8-return
+  "RET501",
+  "RET502",
+  "RET503",
+  "RET504",
+  "RET505",
+  "RET506",
+  "RET507",
+  "RET508",
+  # flake8-self
+  "SLF001",
+  # flake8-simplify
+  "SIM101",
+  "SIM102",
+  "SIM103",
+  "SIM105",
+  "SIM107",
+  "SIM108",
+  "SIM109",
+  "SIM110",
+  "SIM112",
+  "SIM113",
+  "SIM114",
+  "SIM115",
+  "SIM116",
+  "SIM117",
+  "SIM118",
+  "SIM201",
+  "SIM202",
+  "SIM208",
+  "SIM210",
+  "SIM211",
+  "SIM212",
+  "SIM220",
+  "SIM221",
+  "SIM222",
+  "SIM223",
+  "SIM300",
+  "SIM401",
+  "SIM905",
+  "SIM910",
+  "SIM911",
+  # flake8-slots
+  "SLOT000",
+  "SLOT001",
+  "SLOT002",
+  # flake8-tidy-imports
+  "TID251",
+  "TID252",
+  "TID253",
+  # flake8-todos
+  "TD001",
+  "TD002",
+  "TD003",
+  "TD004",
+  "TD005",
+  "TD006",
+  "TD007",
+  # flake8-type-checking
+  "TC001",
+  "TC002",
+  "TC003",
+  "TC004",
+  "TC005",
+  "TC006",
+  "TC007",
+  "TC010",
+  # flake8-unused-arguments
+  "ARG001",
+  "ARG002",
+  "ARG003",
+  "ARG004",
+  "ARG005",
+  # flake8-use-pathlib
+  "PTH100",
+  "PTH101",
+  "PTH102",
+  "PTH103",
+  "PTH104",
+  "PTH105",
+  "PTH106",
+  "PTH107",
+  "PTH108",
+  "PTH109",
+  "PTH110",
+  "PTH111",
+  "PTH112",
+  "PTH113",
+  "PTH114",
+  "PTH115",
+  "PTH116",
+  "PTH117",
+  "PTH118",
+  "PTH119",
+  "PTH120",
+  "PTH121",
+  "PTH122",
+  "PTH123",
+  "PTH124",
+  "PTH201",
+  "PTH202",
+  "PTH203",
+  "PTH204",
+  "PTH205",
+  "PTH206",
+  "PTH207",
+  "PTH208",
+  "PTH210",
+  "PTH211",
+  # flynt
+  "FLY002",
+  # isort
+  "I001",
+  "I002",
+  # mccabe
+  "C901",
+  # NumPy-specific rules
+  "NPY001",
+  "NPY002",
+  "NPY003",
+  "NPY201",
+  # pep8-naming
+  "N801",
+  "N802",
+  "N803",
+  "N804",
+  "N805",
+  "N806",
+  "N807",
+  "N811",
+  "N812",
+  "N813",
+  "N814",
+  "N815",
+  "N816",
+  "N817",
+  "N818",
+  "N999",
+  # pandas-vet
+  "PD002",
+  "PD003",
+  "PD004",
+  "PD007",
+  "PD008",
+  "PD009",
+  "PD010",
+  "PD011",
+  "PD012",
+  "PD013",
+  "PD015",
+  "PD101",
+  # Perflint
+  "PERF101",
+  "PERF102",
+  "PERF203",
+  "PERF401",
+  "PERF402",
+  "PERF403",
+  # pycodestyle
+  "E101",
+  "E401",
+  "E402",
+  "E701",
+  "E702",
+  "E703",
+  "E711",
+  "E712",
+  "E713",
+  "E714",
+  "E721",
+  "E722",
+  "E731",
+  "E741",
+  "E742",
+  "E743",
+  "E902",
+  "W291",
+  "W292",
+  "W293",
+  "W505",
+  "W605",
+  # pydocstyle
+  "D100",
+  "D101",
+  "D102",
+  "D103",
+  "D104",
+  "D105",
+  "D106",
+  "D107",
+  "D200",
+  "D201",
+  "D202",
+  "D204",
+  "D205",
+  "D207",
+  "D208",
+  "D209",
+  "D210",
+  "D211",
+  "D212",
+  "D214",
+  "D215",
+  "D301",
+  "D400",
+  "D401",
+  "D402",
+  "D403",
+  "D404",
+  "D405",
+  "D406",
+  "D407",
+  "D408",
+  "D409",
+  "D410",
+  "D411",
+  "D412",
+  "D413",
+  "D414",
+  "D415",
+  "D417",
+  "D418",
+  "D419",
+  # Pyflakes
+  "F401",
+  "F402",
+  "F403",
+  "F404",
+  "F405",
+  "F406",
+  "F407",
+  "F501",
+  "F502",
+  "F503",
+  "F504",
+  "F505",
+  "F506",
+  "F507",
+  "F508",
+  "F509",
+  "F521",
+  "F522",
+  "F523",
+  "F524",
+  "F525",
+  "F541",
+  "F601",
+  "F602",
+  "F621",
+  "F622",
+  "F631",
+  "F632",
+  "F633",
+  "F634",
+  "F701",
+  "F702",
+  "F704",
+  "F706",
+  "F707",
+  "F722",
+  "F811",
+  "F821",
+  "F822",
+  "F823",
+  "F841",
+  "F842",
+  "F901",
+  # pygrep-hooks
+  "PGH003",
+  "PGH004",
+  "PGH005",
+  # Pylint
+  "PLC0105",
+  "PLC0131",
+  "PLC0132",
+  "PLC0205",
+  "PLC0206",
+  "PLC0207",
+  "PLC0208",
+  "PLC0414",
+  "PLC0415",
+  "PLC1802",
+  "PLC2401",
+  "PLC2403",
+  "PLC3002",
+  "PLE0100",
+  "PLE0101",
+  "PLE0115",
+  "PLE0116",
+  "PLE0117",
+  "PLE0118",
+  "PLE0237",
+  "PLE0241",
+  "PLE0302",
+  "PLE0303",
+  "PLE0304",
+  "PLE0305",
+  "PLE0307",
+  "PLE0308",
+  "PLE0309",
+  "PLE0604",
+  "PLE0605",
+  "PLE0643",
+  "PLE0704",
+  "PLE1132",
+  "PLE1142",
+  "PLE1205",
+  "PLE1206",
+  "PLE1300",
+  "PLE1307",
+  "PLE1310",
+  "PLE1507",
+  "PLE1519",
+  "PLE1520",
+  "PLE1700",
+  "PLE2502",
+  "PLE2510",
+  "PLE2512",
+  "PLE2513",
+  "PLE2514",
+  "PLE2515",
+  "PLR0124",
+  "PLR0133",
+  "PLR0206",
+  "PLR0402",
+  "PLR0911",
+  "PLR0912",
+  "PLR0913",
+  "PLR0915",
+  "PLR0917",
+  "PLR1704",
+  "PLR1708",
+  "PLR1711",
+  "PLR1714",
+  "PLR1716",
+  "PLR1722",
+  "PLR1730",
+  "PLR1733",
+  "PLR1736",
+  "PLR2004",
+  "PLR2044",
+  "PLR5501",
+  "PLW0108",
+  "PLW0120",
+  "PLW0127",
+  "PLW0128",
+  "PLW0129",
+  "PLW0131",
+  "PLW0133",
+  "PLW0177",
+  "PLW0211",
+  "PLW0245",
+  "PLW0406",
+  "PLW0602",
+  "PLW0603",
+  "PLW0604",
+  "PLW0642",
+  "PLW0711",
+  "PLW1501",
+  "PLW1507",
+  "PLW1508",
+  "PLW1509",
+  "PLW1510",
+  "PLW1641",
+  "PLW2101",
+  "PLW2901",
+  "PLW3301",
+  # pyupgrade
+  "UP001",
+  "UP003",
+  "UP004",
+  "UP005",
+  "UP006",
+  "UP007",
+  "UP008",
+  "UP009",
+  "UP010",
+  "UP011",
+  "UP012",
+  "UP013",
+  "UP014",
+  "UP015",
+  "UP017",
+  "UP018",
+  "UP019",
+  "UP020",
+  "UP021",
+  "UP022",
+  "UP023",
+  "UP024",
+  "UP025",
+  "UP026",
+  "UP028",
+  "UP029",
+  "UP030",
+  "UP031",
+  "UP032",
+  "UP033",
+  "UP034",
+  "UP035",
+  "UP036",
+  "UP037",
+  "UP039",
+  "UP040",
+  "UP041",
+  "UP042",
+  "UP043",
+  "UP044",
+  "UP045",
+  "UP046",
+  "UP047",
+  "UP049",
+  "UP050",
+  # refurb
+  "FURB105",
+  "FURB110",
+  "FURB116",
+  "FURB122",
+  "FURB129",
+  "FURB132",
+  "FURB136",
+  "FURB157",
+  "FURB161",
+  "FURB162",
+  "FURB163",
+  "FURB164",
+  "FURB166",
+  "FURB167",
+  "FURB168",
+  "FURB169",
+  "FURB171",
+  "FURB177",
+  "FURB181",
+  "FURB187",
+  "FURB188",
+  "FURB192",
+  # Ruff-specific rules
+  "RUF001",
+  "RUF002",
+  "RUF003",
+  "RUF005",
+  "RUF006",
+  "RUF007",
+  "RUF008",
+  "RUF009",
+  "RUF010",
+  "RUF012",
+  "RUF013",
+  "RUF015",
+  "RUF016",
+  "RUF017",
+  "RUF018",
+  "RUF019",
+  "RUF020",
+  "RUF021",
+  "RUF022",
+  "RUF023",
+  "RUF024",
+  "RUF026",
+  "RUF028",
+  "RUF030",
+  "RUF032",
+  "RUF033",
+  "RUF034",
+  "RUF036",
+  "RUF037",
+  "RUF040",
+  "RUF041",
+  "RUF043",
+  "RUF046",
+  "RUF048",
+  "RUF049",
+  "RUF051",
+  "RUF053",
+  "RUF057",
+  "RUF058",
+  "RUF059",
+  "RUF060",
+  "RUF061",
+  "RUF063",
+  "RUF064",
+  "RUF068",
+  "RUF100",
+  "RUF101",
+  "RUF102",
+  "RUF103",
+  "RUF104",
+  "RUF200",
+  # tryceratops
+  "TRY002",
+  "TRY003",
+  "TRY004",
+  "TRY201",
+  "TRY203",
+  "TRY300",
+  "TRY301",
+  "TRY400",
+  "TRY401",
+]
+preview = false
+
+[lint.per-file-ignores]
+"*_test.py" = ["S101", "PLR2004", "ARG"]
+"test_*.py" = ["S101", "PLR2004", "ARG"]
```

## shfmt

Source: dx. Native file: `.editorconfig`.

```diff
--- a/core/quality/tiers/shfmt/strict.editorconfig
+++ b/core/quality/tiers/shfmt/strict.editorconfig
@@ -1,3 +1,4 @@
-# shfmt defaults, written down so no other .editorconfig is read.
+# Two-space indentation.
 [*]
-indent_style = tab
+indent_style = space
+indent_size = 2
```

## sqlfluff

Source: dx. Native file: `.sqlfluff`.

```diff
--- a/core/quality/tiers/sqlfluff/strict.cfg
+++ b/core/quality/tiers/sqlfluff/strict.cfg
@@ -1,3 +1,3 @@
 # The dialect comes from sql_library, not from this file.
 [sqlfluff]
-rules = layout, ambiguous, references, structure
+rules = all
```

## stylelint

Source: dx. Native file: `.stylelintrc.json, .stylelintrc*, stylelint.config.*`.

```diff
--- a/core/quality/tiers/stylelint/strict.json
+++ b/core/quality/tiers/stylelint/strict.json
@@ -5,8 +5,10 @@
     "at-rule-descriptor-value-no-unknown": true,
     "at-rule-no-deprecated": true,
     "at-rule-no-unknown": true,
+    "at-rule-no-vendor-prefix": true,
     "at-rule-prelude-no-invalid": [true, { "ignoreAtRules": ["media"] }],
     "block-no-empty": true,
+    "block-no-redundant-nested-style-rules": true,
     "color-no-invalid-hex": true,
     "comment-no-empty": true,
     "custom-property-no-missing-var-function": true,
@@ -15,15 +17,22 @@
       true,
       { "ignore": ["consecutive-duplicates-with-different-syntaxes"] }
     ],
+    "declaration-block-no-redundant-longhand-properties": true,
     "declaration-block-no-shorthand-property-overrides": true,
+    "declaration-no-important": true,
     "declaration-property-value-keyword-no-deprecated": true,
     "declaration-property-value-no-unknown": true,
     "font-family-no-duplicate-names": true,
     "font-family-no-missing-generic-family-keyword": true,
     "function-calc-no-unspaced-operator": true,
+    "function-linear-gradient-no-nonstandard-direction": true,
+    "function-no-unknown": true,
+    "function-url-no-scheme-relative": true,
     "keyframe-block-no-duplicate-selectors": true,
     "keyframe-declaration-no-important": true,
+    "length-zero-no-unit": true,
     "media-feature-name-no-unknown": true,
+    "media-feature-name-no-vendor-prefix": true,
     "media-feature-name-value-no-unknown": true,
     "media-query-no-invalid": true,
     "media-type-no-deprecated": true,
@@ -37,6 +46,9 @@
     "no-invalid-position-at-import-rule": true,
     "no-invalid-position-declaration": true,
     "no-irregular-whitespace": true,
+    "no-unknown-animations": true,
+    "no-unknown-custom-media": true,
+    "no-unknown-custom-properties": true,
     "property-no-deprecated": true,
     "property-no-unknown": [
       true,
@@ -45,7 +57,12 @@
         "ignoreSelectors": [":export", "/^:import/"]
       }
     ],
+    "property-no-vendor-prefix": true,
     "selector-anb-no-unmatchable": true,
+    "selector-no-deprecated": true,
+    "selector-no-invalid": true,
+    "selector-no-qualifying-type": true,
+    "selector-no-vendor-prefix": true,
     "selector-pseudo-class-no-unknown": [
       true,
       {
@@ -54,12 +71,14 @@
     ],
     "selector-pseudo-element-no-unknown": true,
     "selector-type-no-unknown": [true, { "ignore": ["custom-elements"] }],
+    "shorthand-property-no-redundant-values": true,
     "string-no-newline": [
       true,
       { "ignore": ["at-rule-preludes", "declaration-values"] }
     ],
     "syntax-string-no-invalid": true,
-    "unit-no-unknown": true
+    "unit-no-unknown": true,
+    "value-no-vendor-prefix": true
   },
   "overrides": [
     { "files": ["*.scss"], "customSyntax": "postcss-scss" },
```

## taplo

Source: dx. Native file: `.taplo.toml`.

Same as the standard tier.

## tsc

Source: dx. Native file: `tsconfig.json`.

```diff
--- a/core/quality/tiers/tsconfig/strict.json
+++ b/core/quality/tiers/tsconfig/strict.json
@@ -1,1 +1,11 @@
-{ "compilerOptions": { "strict": true } }
+{
+  "compilerOptions": {
+    "strict": true,
+    "noUncheckedIndexedAccess": true,
+    "exactOptionalPropertyTypes": true,
+    "noFallthroughCasesInSwitch": true,
+    "noImplicitOverride": true,
+    "noUnusedLocals": true,
+    "noUnusedParameters": true
+  }
+}
```

## typos

Source: dx. Native file: `typos.toml`.

Same as the standard tier.

## yamlfmt

Source: dx. Native file: `.yamlfmt`.

Same as the standard tier.

## yamllint

Source: dx. Native file: `.yamllint`.

```diff
--- a/core/quality/tiers/yamllint/strict.yaml
+++ b/core/quality/tiers/yamllint/strict.yaml
@@ -1,6 +1,23 @@
 rules:
-  anchors: enable
-  key-duplicates: enable
+  anchors:
+    forbid-duplicated-anchors: true
+    forbid-undeclared-aliases: true
+    forbid-unused-anchors: true
+  document-start: enable
+  empty-values: enable
+  float-values:
+    forbid-inf: true
+    forbid-nan: true
+    forbid-scientific-notation: true
+    require-numeral-before-decimal: true
+  key-duplicates:
+    forbid-duplicated-merge-keys: true
+  key-ordering: enable
+  octal-values:
+    forbid-explicit-octal: true
+    forbid-implicit-octal: true
+  quoted-strings:
+    required: only-when-needed
   truthy:
     allowed-values: ['true', 'false']
-    check-keys: false
+    check-keys: true
```
