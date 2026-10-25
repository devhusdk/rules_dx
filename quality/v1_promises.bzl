"""V1 acceptance promises owned by //quality."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_cc_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_cc_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:consumer_policy_cue_formatted_test",
            "//quality:support_unit",
        ],
        "id": "aspect.real_cue_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:csharp_consumer_test",
        ],
        "id": "aspect.real_dotnet_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:csharp_consumer_test",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "aspect.real_dotnet_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:html_template_consumer_test",
        ],
        "id": "aspect.real_html_template_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:html_template_consumer_test",
        ],
        "id": "aspect.real_html_template_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_js_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_js_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:consumer_policy_jsonnet_formatted_test",
            "//quality:support_unit",
        ],
        "id": "aspect.real_jsonnet_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_jvm_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_jvm_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:powershell_consumer_test",
        ],
        "id": "aspect.real_powershell_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "aspect.real_protobuf_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "aspect.real_protobuf_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_python_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:ruby_consumer_test",
        ],
        "id": "aspect.real_ruby_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:ruby_consumer_test",
        ],
        "id": "aspect.real_ruby_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_rust_typecheck_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_scala_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_scala_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_shell_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_shell_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:consumer_policy_terraform_formatted_test",
            "//quality:support_unit",
        ],
        "id": "aspect.real_terraform_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_text_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
        ],
        "id": "aspect.real_typecheck_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:yaml_consumer_test",
        ],
        "id": "aspect.real_yaml_format_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:real_pipeline_unit",
            "//quality:support_unit",
            "//quality/tests:yaml_consumer_test",
        ],
        "id": "aspect.real_yaml_lint_aspect",
        "native": True,
        "owner": "//quality:real_aspects.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.astro",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.c",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cpp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.csharp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.css",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cuda",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.cue",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.elixir",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.erlang",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.fsharp",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.gherkin",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.gleam",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.go",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.go_module",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.graphql",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.html",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.html_template",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.java",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.javascript",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.json",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.json5",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsonc",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsonnet",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.jsx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.kotlin",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.less",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.markdown",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.mdx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.pkl",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.powershell",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.protobuf",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.python",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.python_stub",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.qml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.ruby",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.rust",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.scala",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.scss",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.shell",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.sql",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.starlark",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.svelte",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.terraform",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.text",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.toml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.tsx",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.typescript",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.vue",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.xml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.yaml",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:sources_registry",
        ],
        "id": "class.zig",
        "native": False,
        "owner": "//quality:sources.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.cc",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.graphql",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.html",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.java",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.javascript",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.json",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.kotlin",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.markdown",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.python",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.rust",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.scala",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.shell",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.starlark",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.text",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.toml",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:curated_defaults_unit",
        ],
        "id": "curated.typescript",
        "native": False,
        "owner": "//quality:curated_defaults.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.astro",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.mdx",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.svelte",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:parity_unit",
        ],
        "id": "deferred.vue",
        "native": False,
        "owner": "ADR",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.gherkin",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.go_module",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.json",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.markdown",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.pkl",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.protobuf",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.shell",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.sql",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.starlark",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.text",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.toml",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.xml",
        "native": False,
        "owner": "other",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.biome",
        "native": True,
        "owner": "@dx_tools//:biome",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:buf_consumer_test",
        ],
        "id": "tool.buf",
        "native": True,
        "owner": "@dx_tools//:buf",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.buildifier",
        "native": True,
        "owner": "@dx_tools//:buildifier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clang_format",
        "native": True,
        "owner": "@llvm//tools:clang-format",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clang_tidy",
        "native": True,
        "owner": "@llvm//tools:clang-tidy",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.cppcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality/testdata:matrix_cue_format_fail",
            "//quality/testdata:matrix_cue_format_module",
            "//quality/testdata:matrix_cue_format_module_syntax_error",
            "//quality/testdata:matrix_cue_format_syntax_error",
            "//quality/tests:cue_consumer_test",
        ],
        "id": "tool.cue",
        "native": True,
        "owner": "@dx_tools//:cue",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/testdata:erlfmt_flag_set_test",
            "//quality/testdata:matrix_erlang_format_syntax",
            "//quality/tests:erlfmt_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.erlfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.errcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/testdata:gleam_format_flag_set_test",
            "//quality/testdata:matrix_gleam_format_syntax",
            "//quality/tests:gleam_format_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.gleam_format",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.gofumpt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.govet",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality/testdata:matrix_jsonnet_format_fail",
            "//quality/testdata:matrix_jsonnet_format_module",
            "//quality/testdata:matrix_jsonnet_format_module_syntax_error",
            "//quality/testdata:matrix_jsonnet_format_pass",
            "//quality/testdata:matrix_jsonnet_format_syntax_error",
            "//quality/tests:jsonnet_consumer_test",
        ],
        "id": "tool.jsonnetfmt",
        "native": True,
        "owner": "@dx_tools//:jsonnetfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.keep_sorted",
        "native": True,
        "owner": "@dx_tools//:keep_sorted",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/testdata:matrix_elixir_format_syntax",
            "//quality/testdata:mix_format_flag_set_test",
            "//quality/tests:mix_format_consumer_test",
            "//quality:registry_unit",
        ],
        "id": "tool.mix_format",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.modfmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.pkl",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/tests:qml_format_consumer_test",
            "//quality/tests:qt_adapter_test",
            "//quality:registry_unit",
        ],
        "id": "tool.qmlformat",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/tests:qml_lint_consumer_test",
            "//quality/tests:qt_adapter_test",
            "//quality:registry_unit",
        ],
        "id": "tool.qmllint",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:support_unit",
            "//quality/tests:csharp_consumer_test",
        ],
        "id": "tool.roslyn",
        "native": True,
        "owner": "//quality:roslyn.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ruff",
        "native": True,
        "owner": "@dx_tools//:ruff",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.scalafmt",
        "native": True,
        "owner": "@dx_tools//:scalafmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.shellcheck",
        "native": True,
        "owner": "@dx_tools//:shellcheck",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.shfmt",
        "native": True,
        "owner": "@dx_tools//:shfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
        ],
        "id": "tool.staticcheck",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "gapped",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.taplo",
        "native": True,
        "owner": "@dx_tools//:taplo",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality/testdata:matrix_terraform_format_fail",
            "//quality/testdata:matrix_terraform_format_module",
            "//quality/testdata:matrix_terraform_format_module_syntax_error",
            "//quality/testdata:matrix_terraform_format_syntax_error",
            "//quality/tests:terraform_consumer_test",
        ],
        "id": "tool.terraform",
        "native": True,
        "owner": "@dx_tools//:terraform",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ty",
        "native": True,
        "owner": "@dx_tools//:ty",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.vale",
        "native": True,
        "owner": "@dx_tools//:vale",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:yaml_consumer_test",
        ],
        "id": "tool.yamlfmt",
        "native": True,
        "owner": "@dx_tools//:yamlfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:registry_unit",
            "//quality/testdata:zig_fmt_flag_set_test",
            "//quality/tests:zig_fmt_consumer_test",
        ],
        "id": "tool.zig_fmt",
        "native": True,
        "owner": "//quality:adapters.bzl",
        "state": "evidenced",
    },
]
