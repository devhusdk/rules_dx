# Standard tiers

This page lists the config each tool gets at the standard tier. This page is generated. To refresh it, run `bazel run //docs:update_tiers`.

## buf

Source: dx. Native file: `buf.yaml`.

```yaml
version: v2
lint:
  use:
    - STANDARD
breaking:
  use:
    - WIRE_JSON
```

## buildifier

Source: dx. Native file: `.buildifier.json`.

```json
{
  "warnings": "default"
}
```

## checkmake

Source: dx. Native file: `checkmake.ini`.

```ini
[maxbodylength]
disabled=true

[minphony]
disabled=false

[phonydeclared]
disabled=false

[timestampexpanded]
disabled=false

[uniquetargets]
disabled=false
```

## cmakelint

Source: dx. Native file: `.cmakelintrc`.

```cmakelintrc
# gersemi owns layout, so the whitespace and line length checks are off.
filter=-linelength,-whitespace/eol,-whitespace/extra,-whitespace/indent,-whitespace/mismatch,-whitespace/newline,-whitespace/tabs,-convention/filename
```

## dart

Source: dx. Native file: `analysis_options.yaml`.

```yaml
# Rules of package:lints 6.1.0 recommended.yaml (which includes core.yaml), inlined.
linter:
  rules:
    - annotate_overrides
    - avoid_empty_else
    - avoid_function_literals_in_foreach_calls
    - avoid_init_to_null
    - avoid_relative_lib_imports
    - avoid_renaming_method_parameters
    - avoid_returning_null_for_void
    - avoid_return_types_on_setters
    - avoid_shadowing_type_parameters
    - avoid_single_cascade_in_expression_statements
    - avoid_types_as_parameter_names
    - await_only_futures
    - camel_case_extensions
    - camel_case_types
    - collection_methods_unrelated_type
    - constant_identifier_names
    - control_flow_in_finally
    - curly_braces_in_flow_control_structures
    - dangling_library_doc_comments
    - depend_on_referenced_packages
    - empty_catches
    - empty_constructor_bodies
    - empty_statements
    - exhaustive_cases
    - file_names
    - hash_and_equals
    - implementation_imports
    - implicit_call_tearoffs
    - invalid_runtime_check_with_js_interop_types
    - library_annotations
    - library_prefixes
    - library_private_types_in_public_api
    - no_duplicate_case_values
    - no_leading_underscores_for_library_prefixes
    - no_leading_underscores_for_local_identifiers
    - non_constant_identifier_names
    - no_wildcard_variable_uses
    - null_check_on_nullable_type_parameter
    - null_closures
    - overridden_fields
    - package_names
    - prefer_adjacent_string_concatenation
    - prefer_collection_literals
    - prefer_conditional_assignment
    - prefer_contains
    - prefer_final_fields
    - prefer_for_elements_to_map_fromIterable
    - prefer_function_declarations_over_variables
    - prefer_generic_function_type_aliases
    - prefer_if_null_operators
    - prefer_initializing_formals
    - prefer_inlined_adds
    - prefer_interpolation_to_compose_strings
    - prefer_is_empty
    - prefer_is_not_empty
    - prefer_is_not_operator
    - prefer_iterable_whereType
    - prefer_null_aware_operators
    - prefer_spread_collections
    - prefer_typing_uninitialized_variables
    - provide_deprecation_message
    - recursive_getters
    - secure_pubspec_urls
    - slash_for_doc_comments
    - strict_top_level_inference
    - type_init_formals
    - type_literal_in_constant_pattern
    - unintended_html_in_doc_comment
    - unnecessary_brace_in_string_interps
    - unnecessary_const
    - unnecessary_constructor_name
    - unnecessary_getters_setters
    - unnecessary_late
    - unnecessary_library_name
    - unnecessary_new
    - unnecessary_nullable_for_final_variable_declarations
    - unnecessary_null_aware_assignments
    - unnecessary_null_in_if_null_operators
    - unnecessary_overrides
    - unnecessary_string_escapes
    - unnecessary_string_interpolations
    - unnecessary_this
    - unnecessary_to_list_in_spreads
    - unnecessary_underscores
    - unrelated_type_equality_checks
    - use_function_type_syntax_for_parameters
    - use_null_aware_elements
    - use_rethrow_when_possible
    - use_string_in_part_of_directives
    - use_super_parameters
    - valid_regexps
    - void_checks
```

## docstrfmt

Source: dx. Native file: `docstrfmt.toml`.

```toml
[tool.docstrfmt]
bullet-list-marker = "-"
format-python-code-blocks = true
indent-width = 3
line-length = 88
```

## eslint

Source: dx. Native file: `eslint.config.mjs, eslint.config.js, eslint.config.cjs, eslint.config.ts, .eslintrc*`.

```mjs
import { defineConfig } from "eslint/config";
import globals from "globals";
import tseslint from "typescript-eslint";
import frameworks from "./frameworks.mjs";

const typed = process.env.DX_ESLINT_TYPED === "1";
const typescriptFiles = ["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.cts"];

const core = {
	"constructor-super": "error",
	"for-direction": "error",
	"getter-return": "error",
	"no-async-promise-executor": "error",
	"no-case-declarations": "error",
	"no-class-assign": "error",
	"no-compare-neg-zero": "error",
	"no-cond-assign": "error",
	"no-const-assign": "error",
	"no-constant-binary-expression": "error",
	"no-constant-condition": "error",
	"no-control-regex": "error",
	"no-debugger": "error",
	"no-delete-var": "error",
	"no-dupe-args": "error",
	"no-dupe-class-members": "error",
	"no-dupe-else-if": "error",
	"no-dupe-keys": "error",
	"no-duplicate-case": "error",
	"no-empty": "error",
	"no-empty-character-class": "error",
	"no-empty-pattern": "error",
	"no-empty-static-block": "error",
	"no-ex-assign": "error",
	"no-extra-boolean-cast": "error",
	"no-fallthrough": "error",
	"no-func-assign": "error",
	"no-global-assign": "error",
	"no-import-assign": "error",
	"no-invalid-regexp": "error",
	"no-irregular-whitespace": "error",
	"no-loss-of-precision": "error",
	"no-misleading-character-class": "error",
	"no-new-native-nonconstructor": "error",
	"no-nonoctal-decimal-escape": "error",
	"no-obj-calls": "error",
	"no-octal": "error",
	"no-prototype-builtins": "error",
	"no-redeclare": "error",
	"no-regex-spaces": "error",
	"no-self-assign": "error",
	"no-setter-return": "error",
	"no-shadow-restricted-names": "error",
	"no-sparse-arrays": "error",
	"no-this-before-super": "error",
	"no-unassigned-vars": "error",
	"no-undef": "error",
	"no-unexpected-multiline": "error",
	"no-unreachable": "error",
	"no-unsafe-finally": "error",
	"no-unsafe-negation": "error",
	"no-unsafe-optional-chaining": "error",
	"no-unused-labels": "error",
	"no-unused-private-class-members": "error",
	"no-unused-vars": "error",
	"no-useless-assignment": "error",
	"no-useless-backreference": "error",
	"no-useless-catch": "error",
	"no-useless-escape": "error",
	"no-with": "error",
	"preserve-caught-error": "error",
	"require-yield": "error",
	"use-isnan": "error",
	"valid-typeof": "error",
};

const typescript = {
	"constructor-super": "off",
	"getter-return": "off",
	"no-class-assign": "off",
	"no-const-assign": "off",
	"no-dupe-args": "off",
	"no-dupe-class-members": "off",
	"no-dupe-keys": "off",
	"no-func-assign": "off",
	"no-import-assign": "off",
	"no-new-native-nonconstructor": "off",
	"no-new-symbol": "off",
	"no-obj-calls": "off",
	"no-redeclare": "off",
	"no-setter-return": "off",
	"no-this-before-super": "off",
	"no-undef": "off",
	"no-unreachable": "off",
	"no-unsafe-negation": "off",
	"no-var": "error",
	"no-with": "off",
	"prefer-const": "error",
	"prefer-rest-params": "error",
	"prefer-spread": "error",
	"@typescript-eslint/await-thenable": "error",
	"@typescript-eslint/ban-ts-comment": "error",
	"no-array-constructor": "off",
	"@typescript-eslint/no-array-constructor": "error",
	"@typescript-eslint/no-array-delete": "error",
	"@typescript-eslint/no-base-to-string": "error",
	"@typescript-eslint/no-duplicate-enum-values": "error",
	"@typescript-eslint/no-duplicate-type-constituents": "error",
	"@typescript-eslint/no-empty-object-type": "error",
	"@typescript-eslint/no-explicit-any": "error",
	"@typescript-eslint/no-extra-non-null-assertion": "error",
	"@typescript-eslint/no-floating-promises": "error",
	"@typescript-eslint/no-for-in-array": "error",
	"no-implied-eval": "off",
	"@typescript-eslint/no-implied-eval": "error",
	"@typescript-eslint/no-misused-new": "error",
	"@typescript-eslint/no-misused-promises": "error",
	"@typescript-eslint/no-namespace": "error",
	"@typescript-eslint/no-non-null-asserted-optional-chain": "error",
	"@typescript-eslint/no-redundant-type-constituents": "error",
	"@typescript-eslint/no-require-imports": "error",
	"@typescript-eslint/no-this-alias": "error",
	"@typescript-eslint/no-unnecessary-type-assertion": "error",
	"@typescript-eslint/no-unnecessary-type-constraint": "error",
	"@typescript-eslint/no-unsafe-argument": "error",
	"@typescript-eslint/no-unsafe-assignment": "error",
	"@typescript-eslint/no-unsafe-call": "error",
	"@typescript-eslint/no-unsafe-declaration-merging": "error",
	"@typescript-eslint/no-unsafe-enum-comparison": "error",
	"@typescript-eslint/no-unsafe-function-type": "error",
	"@typescript-eslint/no-unsafe-member-access": "error",
	"@typescript-eslint/no-unsafe-return": "error",
	"@typescript-eslint/no-unsafe-unary-minus": "error",
	"no-unused-expressions": "off",
	"@typescript-eslint/no-unused-expressions": "error",
	"no-unused-vars": "off",
	"@typescript-eslint/no-unused-vars": "error",
	"@typescript-eslint/no-wrapper-object-types": "error",
	"no-throw-literal": "off",
	"@typescript-eslint/only-throw-error": "error",
	"@typescript-eslint/prefer-as-const": "error",
	"@typescript-eslint/prefer-namespace-keyword": "error",
	"prefer-promise-reject-errors": "off",
	"@typescript-eslint/prefer-promise-reject-errors": "error",
	"require-await": "off",
	"@typescript-eslint/require-await": "error",
	"@typescript-eslint/restrict-plus-operands": "error",
	"@typescript-eslint/restrict-template-expressions": "error",
	"@typescript-eslint/triple-slash-reference": "error",
	"@typescript-eslint/unbound-method": "error",
};

export default defineConfig(
	{ languageOptions: { globals: { ...globals.browser, ...globals.node } } },
	{
		files: ["**/*.js", "**/*.jsx", "**/*.mjs", "**/*.cjs", ...typescriptFiles],
		rules: core,
	},
	{
		files: typescriptFiles,
		extends: [
			tseslint.configs.base,
			{ rules: typescript },
			...(typed ? [] : [tseslint.configs.disableTypeChecked]),
		],
		languageOptions: typed
			? {
					parserOptions: {
						projectService: true,
						tsconfigRootDir: process.cwd(),
					},
				}
			: {},
	},
	frameworks,
);
```

## gersemi

Source: dx. Native file: `.gersemirc`.

```yaml
indent: 4
line_length: 80
list_expansion: favour-inlining
sort_order: case-sensitive
unsafe: true
warn_about_unknown_commands: false
```

## hlint

Source: dx. Native file: `.hlint.yaml`.

```yaml
- ignore: {name: Use camelCase}
- ignore: {name: Use section}
- ignore: {name: Use lambda-case}
- ignore: {name: Use tuple-section}
- ignore: {name: Redundant bracket}
- ignore: {name: Move brackets to avoid $}
```

## lychee

Source: dx. Native file: `lychee.toml`.

```toml
# Link checks use lychee's defaults, written down so no other config is read.
timeout = 20
max_retries = 3
max_redirects = 10
```

## mbake

Source: dx. Native file: `.bake.toml`.

```toml
[formatter]
align_variable_assignments = false
auto_insert_phony_declarations = false
ensure_final_newline = true
fix_missing_recipe_tabs = true
group_phony_declarations = false
indent_nested_conditionals = false
max_consecutive_empty_lines = 2
max_line_length = 120
normalize_empty_lines = true
normalize_line_continuations = true
phony_at_top = false
remove_trailing_whitespace = true
space_after_colon = true
space_around_assignment = true
space_before_colon = false
tab_width = 2
```

## mix_format

Source: dx. Native file: `.formatter.exs`.

```exs
# mix format defaults, written down so no other .formatter.exs is read.
[
  line_length: 98
]
```

## perltidy

Source: dx. Native file: `.perltidyrc`.

```perltidyrc
# perltidy defaults, written down so no other profile is read.
--indent-columns=4
--maximum-line-length=80
```

## psscriptanalyzer

Source: dx. Native file: `PSScriptAnalyzerSettings.psd1`.

```psd1
@{
    IncludeDefaultRules = $true
    Rules               = @{
        PSUseConsistentIndentation = @{ Enable = $true }
    }
}
```

## pydoclint

Source: dx. Native file: `*.toml`.

```toml
# Numpy-style docstrings. Short docstrings are not checked.
[tool.pydoclint]
style = "numpy"
skip-checking-short-docstrings = true
check-arg-order = true
```

## ruff

Source: dx. Native file: `ruff.toml, .ruff.toml`.

```toml
# Syntax errors, pyflakes, bugbear, security, deprecated APIs, performance and unused code.
# No style rules. Codes come from the pinned `ruff rule --all --output-format json`.
[lint]
select = [
  # syntax errors
  "E902",
  # pyflakes
  "F401",
  "F402",
  "F403",
  "F404",
  "F405",
  "F406",
  "F407",
  "F501",
  "F502",
  "F503",
  "F504",
  "F505",
  "F506",
  "F507",
  "F508",
  "F509",
  "F521",
  "F522",
  "F523",
  "F524",
  "F525",
  "F541",
  "F601",
  "F602",
  "F621",
  "F622",
  "F631",
  "F632",
  "F633",
  "F634",
  "F701",
  "F702",
  "F704",
  "F706",
  "F707",
  "F722",
  "F811",
  "F821",
  "F822",
  "F823",
  "F841",
  "F842",
  "F901",
  # bugbear
  "B002",
  "B003",
  "B004",
  "B005",
  "B006",
  "B007",
  "B008",
  "B009",
  "B010",
  "B011",
  "B012",
  "B013",
  "B014",
  "B015",
  "B016",
  "B017",
  "B018",
  "B019",
  "B020",
  "B021",
  "B022",
  "B023",
  "B024",
  "B025",
  "B026",
  "B027",
  "B028",
  "B029",
  "B030",
  "B031",
  "B032",
  "B033",
  "B034",
  "B035",
  "B039",
  "B904",
  "B905",
  "B911",
  "B912",
  # security
  "S101",
  "S102",
  "S103",
  "S104",
  "S105",
  "S106",
  "S107",
  "S108",
  "S110",
  "S112",
  "S113",
  "S201",
  "S202",
  "S301",
  "S302",
  "S303",
  "S304",
  "S305",
  "S306",
  "S307",
  "S308",
  "S310",
  "S311",
  "S312",
  "S313",
  "S314",
  "S315",
  "S316",
  "S317",
  "S318",
  "S319",
  "S321",
  "S323",
  "S324",
  "S501",
  "S502",
  "S503",
  "S504",
  "S505",
  "S506",
  "S507",
  "S508",
  "S509",
  "S601",
  "S602",
  "S603",
  "S604",
  "S605",
  "S606",
  "S607",
  "S608",
  "S609",
  "S610",
  "S611",
  "S612",
  "S701",
  "S702",
  "S704",
  # deprecated APIs
  "UP005",
  "UP019",
  "UP026",
  "UP035",
  "UP036",
  # performance
  "PERF101",
  "PERF102",
  "PERF203",
  "PERF401",
  "PERF402",
  "PERF403",
  # unused
  "ARG001",
  "ARG002",
  "ARG003",
  "ARG004",
  "ARG005",
  "RUF059",
]
preview = false

[lint.per-file-ignores]
"*_test.py" = ["S101", "ARG"]
"test_*.py" = ["S101", "ARG"]
```

## shfmt

Source: dx. Native file: `.editorconfig`.

```editorconfig
# shfmt defaults, written down so no other .editorconfig is read.
[*]
indent_style = tab
```

## sqlfluff

Source: dx. Native file: `.sqlfluff`.

```cfg
# The dialect comes from sql_library, not from this file.
[sqlfluff]
rules = layout, ambiguous, references, structure
```

## stylelint

Source: dx. Native file: `.stylelintrc.json, .stylelintrc*, stylelint.config.*`.

```json
{
  "rules": {
    "annotation-no-unknown": true,
    "at-rule-descriptor-no-unknown": true,
    "at-rule-descriptor-value-no-unknown": true,
    "at-rule-no-deprecated": true,
    "at-rule-no-unknown": true,
    "at-rule-prelude-no-invalid": [true, { "ignoreAtRules": ["media"] }],
    "block-no-empty": true,
    "color-no-invalid-hex": true,
    "comment-no-empty": true,
    "custom-property-no-missing-var-function": true,
    "declaration-block-no-duplicate-custom-properties": true,
    "declaration-block-no-duplicate-properties": [
      true,
      { "ignore": ["consecutive-duplicates-with-different-syntaxes"] }
    ],
    "declaration-block-no-shorthand-property-overrides": true,
    "declaration-property-value-keyword-no-deprecated": true,
    "declaration-property-value-no-unknown": true,
    "font-family-no-duplicate-names": true,
    "font-family-no-missing-generic-family-keyword": true,
    "function-calc-no-unspaced-operator": true,
    "keyframe-block-no-duplicate-selectors": true,
    "keyframe-declaration-no-important": true,
    "media-feature-name-no-unknown": true,
    "media-feature-name-value-no-unknown": true,
    "media-query-no-invalid": true,
    "media-type-no-deprecated": true,
    "named-grid-areas-no-invalid": true,
    "nesting-selector-no-missing-scoping-root": true,
    "no-descending-specificity": true,
    "no-duplicate-at-import-rules": true,
    "no-duplicate-selectors": true,
    "no-empty-source": true,
    "no-invalid-double-slash-comments": true,
    "no-invalid-position-at-import-rule": true,
    "no-invalid-position-declaration": true,
    "no-irregular-whitespace": true,
    "property-no-deprecated": true,
    "property-no-unknown": [
      true,
      {
        "ignoreProperties": ["composes", "compose-with"],
        "ignoreSelectors": [":export", "/^:import/"]
      }
    ],
    "selector-anb-no-unmatchable": true,
    "selector-pseudo-class-no-unknown": [
      true,
      {
        "ignorePseudoClasses": [
          "export",
          "import",
          "global",
          "local",
          "external"
        ]
      }
    ],
    "selector-pseudo-element-no-unknown": true,
    "selector-type-no-unknown": [true, { "ignore": ["custom-elements"] }],
    "string-no-newline": [
      true,
      { "ignore": ["at-rule-preludes", "declaration-values"] }
    ],
    "syntax-string-no-invalid": true,
    "unit-no-unknown": true
  },
  "overrides": [
    { "files": ["*.scss"], "customSyntax": "postcss-scss" },
    { "files": ["*.less"], "customSyntax": "postcss-less" }
  ]
}
```

## taplo

Source: dx. Native file: `.taplo.toml`.

```toml
[formatting]
column_width = 80
indent_string = "  "
reorder_keys = false
```

## tsc

Source: dx. Native file: `tsconfig.json`.

```json
{ "compilerOptions": { "strict": true } }
```

## typos

Source: dx. Native file: `typos.toml`.

```toml
[default]
locale = "en"

[files]
extend-exclude = ["*.lock", "*.min.js", "*.svg"]
```

## yamlfmt

Source: dx. Native file: `.yamlfmt`.

```yaml
formatter:
  type: basic
  indent: 2
  retain_line_breaks: false
```

## yamllint

Source: dx. Native file: `.yamllint`.

```yaml
rules:
  anchors: enable
  key-duplicates: enable
  truthy:
    allowed-values: ['true', 'false']
    check-keys: false
```
