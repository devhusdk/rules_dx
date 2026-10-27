"""Explicit tool support records."""

load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")

SUPPORT_SCHEMA_VERSION = 1

TEXT_KEEP_SORTED_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["text"],
    "context": "owned",
    "execution": "direct",
    "family": "text",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "keep_sorted",
    "transport": "batched",
}

SHELL_SHELLCHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["shell"],
    "context": "owned",
    "execution": "direct",
    "family": "shell",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "shellcheck",
    "transport": "single",
}

DOCKERFILE_HADOLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["dockerfile"],
    "context": "owned",
    "execution": "direct",
    "family": "dockerfile",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "hadolint",
    "transport": "batched",
}

SHELL_SHFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["shell"],
    "context": "owned",
    "execution": "direct",
    "family": "shell",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "shfmt",
    "transport": "single",
}

SCALA_SCALAFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["scala"],
    "context": "owned",
    "execution": "direct",
    "family": "scala",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "scalafmt",
    "transport": "batched",
}

SCALA_SCALAFIX_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["scala"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "scala",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "scalafix",
    "transport": "batched",
}

CSHARP_CSHARPIER_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["csharp"],
    "context": "owned",
    "execution": "direct",
    "family": "csharp",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "csharpier",
    "transport": "batched",
}

CSHARP_ROSLYN_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["csharp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "csharp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "roslyn",
    "transport": "batched",
}

FSHARP_FANTOMAS_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["fsharp"],
    "context": "owned",
    "execution": "direct",
    "family": "fsharp",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fantomas",
    "transport": "batched",
}

FSHARP_FSHARPLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["fsharp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "fsharp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fsharplint",
    "transport": "batched",
}

RUBY_RUBOCOP_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["ruby"],
    "context": "owned",
    "execution": "direct",
    "family": "ruby",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "rubocop",
    "transport": "batched",
}

RUBY_STANDARDRB_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["ruby"],
    "context": "owned",
    "execution": "direct",
    "family": "ruby",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "standardrb",
    "transport": "batched",
}

YAML_YAMLFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["yaml"],
    "context": "owned",
    "execution": "direct",
    "family": "yaml",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "yamlfmt",
    "transport": "batched",
}

YAML_YAMLLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["yaml"],
    "context": "owned",
    "execution": "direct",
    "family": "yaml",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "yamllint",
    "transport": "batched",
}

TERRAFORM_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["terraform"],
    "context": "owned",
    "execution": "direct",
    "family": "terraform",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "terraform",
    "transport": "single",
}

PROTOBUF_BUF_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["protobuf"],
    "context": "owned",
    "execution": "direct",
    "family": "protobuf",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "buf",
    "transport": "single",
}

PROTOBUF_BUF_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["protobuf"],
    "context": "owned",
    "execution": "direct",
    "family": "protobuf",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "buf",
    "transport": "batched",
}

CSS_STYLELINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["css", "less", "scss"],
    "context": "owned",
    "execution": "direct",
    "family": "css",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "stylelint",
    "transport": "batched",
}

GRAPHQL_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["graphql"],
    "context": "owned",
    "execution": "direct",
    "family": "graphql",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "graphql_eslint",
    "transport": "batched",
}

SQL_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["sql"],
    "context": "owned",
    "execution": "direct",
    "family": "sql",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

XML_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["xml"],
    "context": "owned",
    "execution": "direct",
    "family": "xml",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

GHERKIN_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["gherkin"],
    "context": "owned",
    "execution": "direct",
    "family": "gherkin",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

CUE_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["cue"],
    "context": "owned",
    "execution": "direct",
    "family": "cue",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "cue",
    "transport": "batched",
}

JSONNET_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["jsonnet"],
    "context": "owned",
    "execution": "direct",
    "family": "jsonnet",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "jsonnetfmt",
    "transport": "batched",
}

POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["powershell"],
    "context": "owned",
    "execution": "direct",
    "family": "powershell",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "psscriptanalyzer",
    "transport": "batched",
}

HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["html_template"],
    "context": "owned",
    "execution": "direct",
    "family": "html_template",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "djlint",
    "transport": "single",
}

HTML_TEMPLATE_DJLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["html_template"],
    "context": "owned",
    "execution": "direct",
    "family": "html_template",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "djlint",
    "transport": "single",
}

PKL_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["pkl"],
    "context": "owned",
    "execution": "direct",
    "family": "pkl",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "pkl",
    "transport": "batched",
}

SUPPORT_RECORDS = [
    SCALA_SCALAFMT_SUPPORT,
    SCALA_SCALAFIX_SUPPORT,
    CSHARP_CSHARPIER_SUPPORT,
    CSHARP_ROSLYN_SUPPORT,
    FSHARP_FANTOMAS_SUPPORT,
    FSHARP_FSHARPLINT_SUPPORT,
    RUBY_RUBOCOP_SUPPORT,
    RUBY_STANDARDRB_SUPPORT,
    YAML_YAMLFMT_SUPPORT,
    YAML_YAMLLINT_SUPPORT,
    HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT,
    HTML_TEMPLATE_DJLINT_LINT_SUPPORT,
    TEXT_KEEP_SORTED_SUPPORT,
    SHELL_SHELLCHECK_SUPPORT,
    SHELL_SHFMT_SUPPORT,
    DOCKERFILE_HADOLINT_SUPPORT,
    TERRAFORM_FORMAT_SUPPORT,
    PROTOBUF_BUF_FORMAT_SUPPORT,
    PROTOBUF_BUF_LINT_SUPPORT,
    CSS_STYLELINT_LINT_SUPPORT,
    GRAPHQL_ESLINT_LINT_SUPPORT,
    SQL_PRETTIER_FORMAT_SUPPORT,
    XML_PRETTIER_FORMAT_SUPPORT,
    GHERKIN_PRETTIER_FORMAT_SUPPORT,
    CUE_FORMAT_SUPPORT,
    JSONNET_FORMAT_SUPPORT,
    PKL_FORMAT_SUPPORT,
    POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT,
]

def _is_canonical_token(text):
    """Reports whether a token uses the canonical alphabet."""
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def support_record_error(record):
    """Validates one support record against the adapter taxonomy."""
    if SUPPORT_SCHEMA_VERSION != 1:
        return "support: unsupported schema v" + str(SUPPORT_SCHEMA_VERSION) + " (want v1)"
    for key in ["artifact", "capability", "classes", "context", "execution", "family", "fix", "platforms", "state", "tool", "transport"]:
        if key not in record:
            return "support: record is missing field '" + key + "'"
    tool = record["tool"]
    capability = record["capability"]
    family = record["family"]
    if not _is_canonical_token(tool):
        return "support: non-canonical tool '" + str(tool) + "'"
    if capability not in ["audit", "format", "lint", "typecheck"]:
        return "support: unknown capability '" + str(capability) + "' for tool '" + tool + "'"
    if not _is_canonical_token(family):
        return "support: non-canonical family '" + str(family) + "'"
    if tool not in REAL_ADAPTERS:
        return "support: unknown tool '" + tool + "': not in the real adapter registry"
    if capability not in REAL_ADAPTERS[tool]:
        return "support: tool '" + tool + "' has no '" + capability + "' capability"
    classes = record["classes"]
    if type(classes) != "list" or len(classes) == 0:
        return "support: tool '" + tool + "' needs a non-empty class list"
    for class_id in classes:
        if not _is_canonical_token(class_id):
            return "support: non-canonical class '" + str(class_id) + "' for tool '" + tool + "'"
        if class_id not in REAL_CLASS_TO_FAMILY:
            return "support: tool '" + tool + "' names unclassified class '" + class_id + "'"
        if REAL_CLASS_TO_FAMILY[class_id] != family:
            return "support: class '" + class_id + "' belongs to family '" + REAL_CLASS_TO_FAMILY[class_id] + "', not '" + family + "'"
        if class_id not in REAL_ADAPTERS[tool][capability]:
            return "support: class '" + class_id + "' is outside '" + tool + " " + capability + "' adapter support"
    if record["execution"] not in ["delegated", "direct"]:
        return "support: tool '" + tool + "' needs execution 'direct' or 'delegated', got '" + str(record["execution"]) + "'"
    if record["fix"] not in ["check_only", "supported"]:
        return "support: tool '" + tool + "' needs fix 'supported' or 'check_only', got '" + str(record["fix"]) + "'"
    if record["context"] not in ["owned", "target_coupled"]:
        return "support: tool '" + tool + "' needs context 'owned' or 'target_coupled', got '" + str(record["context"]) + "'"
    platforms = record["platforms"]
    if type(platforms) != "list" or len(platforms) == 0:
        return "support: tool '" + tool + "' needs a non-empty platform list"
    for platform in platforms:
        if not _is_canonical_token(platform):
            return "support: non-canonical platform '" + str(platform) + "' for tool '" + tool + "'"
    if record["state"] not in ["known", "qualified", "supported"]:
        return "support: tool '" + tool + "' needs state 'known', 'supported' or 'qualified', got '" + str(record["state"]) + "'"
    if record["artifact"] not in ["missing", "pinned"]:
        return "support: tool '" + tool + "' needs artifact 'missing' or 'pinned', got '" + str(record["artifact"]) + "'"
    if record["transport"] not in ["batched", "single"]:
        return "support: tool '" + tool + "' needs transport 'batched' or 'single', got '" + str(record["transport"]) + "'"
    return ""

def support_schema_error():
    """Validates every recorded support entry."""
    for record in SUPPORT_RECORDS:
        err = support_record_error(record)
        if err != "":
            return err
    return ""

def _wiring_error(record, wired_tools, aspect):
    """Reports the missing wired aspect for one qualified record."""
    if record["tool"] in wired_tools:
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no wired aspect (missing " + aspect + "; not silently clean)"

def support_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the migrated family."""
    return _wiring_error(TEXT_KEEP_SORTED_SUPPORT, wired_tools, "real_text_lint_aspect")

def support_shell_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_shell_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_scala_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_scala_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_cue_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the CUE family."""
    return _wiring_error(CUE_FORMAT_SUPPORT, wired_tools, "real_cue_format_aspect")

def support_jsonnet_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Jsonnet family."""
    return _wiring_error(JSONNET_FORMAT_SUPPORT, wired_tools, "real_jsonnet_format_aspect")

def support_pkl_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Pkl family."""
    return _wiring_error(PKL_FORMAT_SUPPORT, wired_tools, "real_pkl_format_aspect")

def support_terraform_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Terraform family."""
    return _wiring_error(TERRAFORM_FORMAT_SUPPORT, wired_tools, "real_terraform_format_aspect")

def support_protobuf_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_protobuf_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_css_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the CSS family."""
    return _wiring_error(CSS_STYLELINT_LINT_SUPPORT, wired_tools, "real_js_lint_aspect")

def support_graphql_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the GraphQL lint family."""
    return _wiring_error(GRAPHQL_ESLINT_LINT_SUPPORT, wired_tools, "real_js_lint_aspect")

def support_prettier_plugins_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Prettier plugin families."""
    for record in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_js_format_aspect")
        if err != "":
            return err
    return ""

def support_dockerfile_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Dockerfile family."""
    return _wiring_error(DOCKERFILE_HADOLINT_SUPPORT, wired_tools, "real_dockerfile_lint_aspect")

def support_powershell_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the PowerShell family."""
    return _wiring_error(POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT, wired_tools, "real_powershell_lint_aspect")

def support_csharp_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the C# family."""
    for record in [CSHARP_CSHARPIER_SUPPORT, CSHARP_ROSLYN_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_dotnet_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_fsharp_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the F# family."""
    for record in [FSHARP_FANTOMAS_SUPPORT, FSHARP_FSHARPLINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_dotnet_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_ruby_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Ruby family."""
    for record in [RUBY_RUBOCOP_SUPPORT, RUBY_STANDARDRB_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_ruby_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_html_template_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the HTML template family."""
    for record in [HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT, HTML_TEMPLATE_DJLINT_LINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_html_template_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_yaml_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the YAML family."""
    for record in [YAML_YAMLFMT_SUPPORT, YAML_YAMLLINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_yaml_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def _cli_error(record, cli_aspects, aspect):
    """Reports the missing CLI aspect for one qualified record."""
    for seen in cli_aspects:
        if aspect in seen:
            return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no CLI " + record["capability"] + " aspect (missing " + aspect + ")"

def support_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the migrated family."""
    return _cli_error(TEXT_KEEP_SORTED_SUPPORT, cli_aspects, "real_text_lint_aspect")

def support_shell_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_shell_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_scala_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_scala_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_cue_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the CUE family."""
    return _cli_error(CUE_FORMAT_SUPPORT, cli_aspects, "real_cue_format_aspect")

def support_jsonnet_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Jsonnet family."""
    return _cli_error(JSONNET_FORMAT_SUPPORT, cli_aspects, "real_jsonnet_format_aspect")

def support_pkl_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Pkl family."""
    return _cli_error(PKL_FORMAT_SUPPORT, cli_aspects, "real_pkl_format_aspect")

def support_terraform_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Terraform family."""
    return _cli_error(TERRAFORM_FORMAT_SUPPORT, cli_aspects, "real_terraform_format_aspect")

def support_protobuf_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_protobuf_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_css_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the CSS family."""
    return _cli_error(CSS_STYLELINT_LINT_SUPPORT, cli_aspects, "real_js_lint_aspect")

def support_graphql_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the GraphQL lint family."""
    return _cli_error(GRAPHQL_ESLINT_LINT_SUPPORT, cli_aspects, "real_js_lint_aspect")

def support_prettier_plugins_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Prettier plugin families."""
    for record in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_js_format_aspect")
        if err != "":
            return err
    return ""

def support_dockerfile_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Dockerfile family."""
    return _cli_error(DOCKERFILE_HADOLINT_SUPPORT, cli_aspects, "real_dockerfile_lint_aspect")

def support_powershell_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the PowerShell family."""
    return _cli_error(POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT, cli_aspects, "real_powershell_lint_aspect")

def support_csharp_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the C# family."""
    for record in [CSHARP_CSHARPIER_SUPPORT, CSHARP_ROSLYN_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_dotnet_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def _artifact_error(record):
    """Reports the missing pinned artifact for one qualified record."""
    if record["artifact"] == "pinned":
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no pinned artifact (quality/artifacts/" + record["tool"] + ".*.bzl is missing)"

def support_artifact_error():
    """Reports the actionable artifact gap for the migrated family."""
    return _artifact_error(TEXT_KEEP_SORTED_SUPPORT)

def support_shell_artifact_error():
    """Reports the actionable artifact gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_scala_artifact_error():
    """Reports the actionable artifact gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_cue_artifact_error():
    """Reports the actionable artifact gap for the CUE family."""
    return _artifact_error(CUE_FORMAT_SUPPORT)

def support_jsonnet_artifact_error():
    """Reports the actionable artifact gap for the Jsonnet family."""
    return _artifact_error(JSONNET_FORMAT_SUPPORT)

def support_pkl_artifact_error():
    """Reports the actionable artifact gap for the Pkl family."""
    return _artifact_error(PKL_FORMAT_SUPPORT)

def support_terraform_artifact_error():
    """Reports the actionable artifact gap for the Terraform family."""
    return _artifact_error(TERRAFORM_FORMAT_SUPPORT)

def support_protobuf_artifact_error():
    """Reports the actionable artifact gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_css_artifact_error():
    """Reports the actionable artifact gap for the CSS family."""
    return _artifact_error(CSS_STYLELINT_LINT_SUPPORT)

def support_graphql_artifact_error():
    """Reports the actionable artifact gap for the GraphQL lint family."""
    return _artifact_error(GRAPHQL_ESLINT_LINT_SUPPORT)

def support_prettier_plugins_artifact_error():
    """Reports the actionable artifact gap for the Prettier plugin families."""
    for record in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_dockerfile_artifact_error():
    """Reports the actionable artifact gap for the Dockerfile family."""
    return _artifact_error(DOCKERFILE_HADOLINT_SUPPORT)

def support_powershell_artifact_error():
    """Reports the actionable artifact gap for the PowerShell family."""
    return _artifact_error(POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT)

def support_csharp_artifact_error():
    """Reports the actionable artifact gap for the C# family."""
    for record in [CSHARP_CSHARPIER_SUPPORT, CSHARP_ROSLYN_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_html_template_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the HTML template family."""
    for record in [HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT, HTML_TEMPLATE_DJLINT_LINT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_html_template_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_html_template_artifact_error():
    """Reports the actionable artifact gap for the HTML template family."""
    for record in [HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT, HTML_TEMPLATE_DJLINT_LINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_yaml_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the YAML family."""
    for record in [YAML_YAMLFMT_SUPPORT, YAML_YAMLLINT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_yaml_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_yaml_artifact_error():
    """Reports the actionable artifact gap for the YAML family."""
    for record in [YAML_YAMLFMT_SUPPORT, YAML_YAMLLINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_ruby_artifact_error():
    """Reports the actionable artifact gap for the Ruby family."""
    for record in [RUBY_RUBOCOP_SUPPORT, RUBY_STANDARDRB_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_fsharp_artifact_error():
    """Reports the actionable artifact gap for the F# family."""
    for record in [FSHARP_FANTOMAS_SUPPORT, FSHARP_FSHARPLINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def is_support_tool(tool_id):
    """Reports whether a tool carries an explicit support record."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return True
    return False

def support_state(tool_id):
    """Returns the recorded support state for one tool, or unknown."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return record["state"]
    return "unknown"
