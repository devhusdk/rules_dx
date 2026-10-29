"""Explicit tool support records."""

load("//quality/artifacts:hub.bzl", "PLATFORM_FALLBACKS", "TOOL_PLATFORMS")
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "hadolint",
    "transport": "batched",
}

ANSIBLE_ANSIBLE_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["ansible"],
    "context": "owned",
    "execution": "direct",
    "family": "ansible",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "ansible_lint",
    "transport": "batched",
    "unsupported": ["windows_x86_64"],
}

BICEP_BICEP_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["bicep"],
    "context": "owned",
    "execution": "direct",
    "family": "bicep",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "bicep",
    "transport": "single",
}

BICEP_BICEP_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["bicep"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "bicep",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "bicep",
    "transport": "single",
}

CLOUDFORMATION_CFN_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["cloudformation"],
    "context": "owned",
    "execution": "direct",
    "family": "cloudformation",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "cfn_lint",
    "transport": "batched",
}

KUBERNETES_KUBECONFORM_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["kubernetes"],
    "context": "owned",
    "execution": "direct",
    "family": "kubernetes",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "kubeconform",
    "transport": "batched",
}

HELM_HELM_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["helm"],
    "context": "owned",
    "execution": "direct",
    "family": "helm",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "helm",
    "transport": "single",
}

MAKE_CHECKMAKE_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["make"],
    "context": "owned",
    "execution": "direct",
    "family": "make",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "checkmake",
    "transport": "batched",
}

MAKE_MBAKE_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["make"],
    "context": "owned",
    "execution": "direct",
    "family": "make",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "mbake",
    "transport": "batched",
}

CMAKE_GERSEMI_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["cmake"],
    "context": "owned",
    "execution": "direct",
    "family": "cmake",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "gersemi",
    "transport": "batched",
}

CMAKE_CMAKELINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["cmake"],
    "context": "owned",
    "execution": "direct",
    "family": "cmake",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "cmakelint",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "scalafmt",
    "transport": "batched",
}

SCALA_SCALAC_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["scala"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "scala",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "scalac",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "scalafix",
    "transport": "batched",
}

JAVA_CHECKSTYLE_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["java"],
    "context": "owned",
    "execution": "direct",
    "family": "java",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "checkstyle",
    "transport": "batched",
}

JAVA_GOOGLE_JAVA_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["java"],
    "context": "owned",
    "execution": "direct",
    "family": "java",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "google_java_format",
    "transport": "batched",
}

JAVA_JAVAC_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["java"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "java",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "javac",
    "transport": "batched",
}

JAVA_PMD_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["java"],
    "context": "owned",
    "execution": "direct",
    "family": "java",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "pmd",
    "transport": "batched",
}

JAVA_SPOTBUGS_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["java"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "java",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "spotbugs",
    "transport": "batched",
}

KOTLIN_KOTLINC_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["kotlin"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "kotlin",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "kotlinc",
    "transport": "batched",
}

KOTLIN_KTFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["kotlin"],
    "context": "owned",
    "execution": "direct",
    "family": "kotlin",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "ktfmt",
    "transport": "batched",
}

KOTLIN_KTLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["kotlin"],
    "context": "owned",
    "execution": "direct",
    "family": "kotlin",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "ktlint",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "roslyn",
    "transport": "batched",
}

CSHARP_CSC_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["csharp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "csharp",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "csc",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "fantomas",
    "transport": "batched",
}

FSHARP_FSC_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["fsharp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "fsharp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fsc",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
    # Ruby portable builds have no Windows, so rubocop is unsupported on windows_x86_64 and windows_arm64.
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
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
    # Ruby portable builds have no Windows, so standardrb is unsupported on windows_x86_64 and windows_arm64.
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "yamllint",
    "transport": "batched",
}

NIX_STATIX_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["nix"],
    "context": "owned",
    "execution": "direct",
    "family": "nix",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "statix",
    "transport": "single",
}

NIX_NIXFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["nix"],
    "context": "owned",
    "execution": "direct",
    "family": "nix",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "nixfmt",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

OCAML_TOPIARY_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["ocaml"],
    "context": "owned",
    "execution": "direct",
    "family": "ocaml",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "topiary",
    "transport": "single",
}

OCAML_ZANUDA_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["ocaml"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "ocaml",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "zanuda",
    "transport": "single",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

NIX_DEADNIX_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["nix"],
    "context": "owned",
    "execution": "direct",
    "family": "nix",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "deadnix",
    "transport": "batched",
}

XML_XMLLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["xml"],
    "context": "owned",
    "execution": "direct",
    "family": "xml",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "xmllint",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "terraform",
    "transport": "single",
}

TERRAFORM_TFLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["terraform"],
    "context": "owned",
    "execution": "direct",
    "family": "terraform",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "tflint",
    "transport": "batched",
}

GO_GOFUMPT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["go"],
    "context": "owned",
    "execution": "direct",
    "family": "go",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "gofumpt",
    "transport": "batched",
}

CC_CLANG_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["c", "cpp"],
    "context": "owned",
    "execution": "direct",
    "family": "cc",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "clang_format",
    "transport": "batched",
}

CUDA_CLANG_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["cuda"],
    "context": "owned",
    "execution": "direct",
    "family": "cuda",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "clang_format",
    "transport": "batched",
}

OBJC_CLANG_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["objc", "objcpp"],
    "context": "owned",
    "execution": "direct",
    "family": "objc",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "clang_format",
    "transport": "batched",
}

CC_CPPCHECK_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["c", "cpp"],
    "context": "owned",
    "execution": "direct",
    "family": "cc",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "cppcheck",
    "transport": "batched",
}

RUST_RUSTFMT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["rust"],
    "context": "owned",
    "execution": "direct",
    "family": "rust",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "rustfmt",
    "transport": "batched",
}

CC_CLANG_TIDY_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["c", "cpp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "cc",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "clang_tidy",
    "transport": "batched",
}

GO_ERRCHECK_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["go"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "go",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "errcheck",
    "transport": "batched",
}

GO_GOVET_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["go"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "go",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "govet",
    "transport": "batched",
}

GO_STATICCHECK_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["go"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "go",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "known",
    "tool": "staticcheck",
    "transport": "batched",
}

RUST_CLIPPY_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["rust"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "rust",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "clippy",
    "transport": "batched",
}

RUST_RUSTC_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["rust"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "rust",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "rustc",
    "transport": "batched",
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

FLATBUFFERS_FLATC_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["flatbuffers"],
    "context": "owned",
    "execution": "direct",
    "family": "flatbuffers",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "flatc",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

FLATBUFFERS_FLATC_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["flatbuffers"],
    "context": "owned",
    "execution": "direct",
    "family": "flatbuffers",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "flatc",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

CAPNP_CAPNP_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["capnp"],
    "context": "owned",
    "execution": "direct",
    "family": "capnp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "capnp",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

CAPNP_CAPNP_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["capnp"],
    "context": "owned",
    "execution": "direct",
    "family": "capnp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "capnp",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

THRIFT_THRIFT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["thrift"],
    "context": "owned",
    "execution": "direct",
    "family": "thrift",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "thrift",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
}

THRIFT_THRIFT_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["thrift"],
    "context": "owned",
    "execution": "direct",
    "family": "thrift",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "thrift",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_x86_64"],
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

API_SPEC_REDOCLY_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["api_spec"],
    "context": "owned",
    "execution": "direct",
    "family": "api_spec",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "redocly",
    "transport": "single",
}

JAVASCRIPT_BIOME_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["javascript", "jsx"],
    "context": "owned",
    "execution": "direct",
    "family": "javascript",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "biome",
    "transport": "batched",
}

JAVASCRIPT_BIOME_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["javascript", "jsx"],
    "context": "owned",
    "execution": "direct",
    "family": "javascript",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "biome",
    "transport": "batched",
}

JAVASCRIPT_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["javascript"],
    "context": "owned",
    "execution": "direct",
    "family": "javascript",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "eslint",
    "transport": "batched",
}

JAVASCRIPT_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["javascript"],
    "context": "owned",
    "execution": "direct",
    "family": "javascript",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

ASTRO_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["astro"],
    "context": "owned",
    "execution": "direct",
    "family": "astro",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "eslint",
    "transport": "batched",
}

SVELTE_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["svelte"],
    "context": "owned",
    "execution": "direct",
    "family": "svelte",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "eslint",
    "transport": "batched",
}

VUE_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["vue"],
    "context": "owned",
    "execution": "direct",
    "family": "vue",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "eslint",
    "transport": "batched",
}

MDX_ESLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["mdx"],
    "context": "owned",
    "execution": "direct",
    "family": "mdx",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "eslint",
    "transport": "batched",
}

TYPESCRIPT_BIOME_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["tsx", "typescript"],
    "context": "owned",
    "execution": "direct",
    "family": "typescript",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "biome",
    "transport": "batched",
}

TYPESCRIPT_BIOME_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["tsx", "typescript"],
    "context": "owned",
    "execution": "direct",
    "family": "typescript",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "biome",
    "transport": "batched",
}

MDX_REMARK_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["mdx"],
    "context": "owned",
    "execution": "direct",
    "family": "mdx",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "remark",
    "transport": "batched",
}

API_SPEC_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["api_spec", "api_spec_ref"],
    "context": "owned",
    "execution": "direct",
    "family": "api_spec",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

ANSIBLE_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["ansible"],
    "context": "owned",
    "execution": "direct",
    "family": "ansible",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

KUBERNETES_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["kubernetes"],
    "context": "owned",
    "execution": "direct",
    "family": "kubernetes",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

DOCKERFILE_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["dockerfile"],
    "context": "owned",
    "execution": "direct",
    "family": "dockerfile",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

PERL_PERLCRITIC_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["perl"],
    "context": "owned",
    "execution": "direct",
    "family": "perl",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "perlcritic",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

PERL_PERLTIDY_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["perl"],
    "context": "owned",
    "execution": "direct",
    "family": "perl",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "perltidy",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

PHP_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["php"],
    "context": "owned",
    "execution": "direct",
    "family": "php",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

PHP_PHPCS_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["php"],
    "context": "owned",
    "execution": "direct",
    "family": "php",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "phpcs",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

PHP_PHPSTAN_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["php"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "php",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "phpstan",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

SQL_PRETTIER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["sql"],
    "context": "owned",
    "execution": "direct",
    "family": "sql",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

SQL_SQLFLUFF_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["sql"],
    "context": "owned",
    "execution": "direct",
    "family": "sql",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "sqlfluff",
    "transport": "batched",
}

SQL_SQLFLUFF_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["sql"],
    "context": "owned",
    "execution": "direct",
    "family": "sql",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "sqlfluff",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "prettier",
    "transport": "batched",
}

GHERKIN_GPLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["gherkin"],
    "context": "owned",
    "execution": "direct",
    "family": "gherkin",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "gplint",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "cue",
    "transport": "batched",
}

CUE_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["cue"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "cue",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "jsonnetfmt",
    "transport": "batched",
}

FORTRAN_FFMT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["fortran"],
    "context": "owned",
    "execution": "direct",
    "family": "fortran",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "ffmt",
    "transport": "batched",
}

FORTRAN_FORTITUDE_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["fortran"],
    "context": "owned",
    "execution": "direct",
    "family": "fortran",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fortitude",
    "transport": "batched",
}

JSONNET_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["jsonnet"],
    "context": "owned",
    "execution": "direct",
    "family": "jsonnet",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "jsonnet_lint",
    "transport": "batched",
}

TYPST_TYPSTYLE_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["typst"],
    "context": "owned",
    "execution": "direct",
    "family": "typst",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "typstyle",
    "transport": "batched",
}

LATEX_LATEXINDENT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["latex"],
    "context": "owned",
    "execution": "direct",
    "family": "latex",
    "fix": "supported",
    "platforms": ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "latexindent",
    "transport": "single",
}

# chktex has no pinned build on any platform yet. Selecting it fails the policy check by name.
LATEX_CHKTEX_LINT_SUPPORT = {
    "artifact": "missing",
    "capability": "lint",
    "classes": ["latex"],
    "context": "owned",
    "execution": "direct",
    "family": "latex",
    "fix": "check_only",
    "platforms": [],
    "state": "known",
    "tool": "chktex",
    "transport": "single",
    "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

# Vale lints AsciiDoc through the pinned Asciidoctor.js converter the owner rule carries.
ASCIIDOC_VALE_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["asciidoc"],
    "context": "owned",
    "execution": "direct",
    "family": "asciidoc",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "vale",
    "transport": "batched",
}

POWERSHELL_PSSCRIPTANALYZER_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["powershell"],
    "context": "owned",
    "execution": "direct",
    "family": "powershell",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "psscriptanalyzer",
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
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
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

# linux_arm64: hlint publishes no linux_arm64 build.
# macos_arm64: hlint v3.10 ships only x86_64-osx and no macOS fallback exists.
HASKELL_HLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["haskell"],
    "context": "owned",
    "execution": "direct",
    "family": "haskell",
    "fix": "check_only",
    "platforms": ["linux_x86_64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "hlint",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64"],
}

# linux_arm64: ormolu publishes no linux_arm64 build.
# macos_arm64: the aarch64-darwin build is not pinned yet (tracked by #4664).
HASKELL_ORMOLU_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["haskell"],
    "context": "owned",
    "execution": "direct",
    "family": "haskell",
    "fix": "supported",
    "platforms": ["linux_x86_64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ormolu",
    "transport": "single",
    "unsupported": ["linux_arm64", "macos_arm64"],
}

PKL_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["pkl"],
    "context": "owned",
    "execution": "direct",
    "family": "pkl",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "pkl",
    "transport": "batched",
}

PKL_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["pkl"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "pkl",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "pkl",
    "transport": "single",
}

LUA_SELENE_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["lua"],
    "context": "owned",
    "execution": "direct",
    "family": "lua",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "selene",
    "transport": "batched",
}

LUA_STYLUA_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["lua"],
    "context": "owned",
    "execution": "direct",
    "family": "lua",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "stylua",
    "transport": "batched",
}

JUPYTER_RUFF_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["jupyter"],
    "context": "owned",
    "execution": "direct",
    "family": "jupyter",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ruff",
    "transport": "batched",
}

JUPYTER_RUFF_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["jupyter"],
    "context": "owned",
    "execution": "direct",
    "family": "jupyter",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ruff",
    "transport": "batched",
}

PYTHON_FLAKE8_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["python", "python_stub"],
    "context": "owned",
    "execution": "direct",
    "family": "python",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "flake8",
    "transport": "batched",
}

PYTHON_PYDOCLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["python", "python_stub"],
    "context": "owned",
    "execution": "direct",
    "family": "python",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "pydoclint",
    "transport": "batched",
}

PYTHON_PYLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["python", "python_stub"],
    "context": "owned",
    "execution": "direct",
    "family": "python",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "pylint",
    "transport": "batched",
}

PYTHON_RUFF_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["python", "python_stub"],
    "context": "owned",
    "execution": "direct",
    "family": "python",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ruff",
    "transport": "batched",
}

PYTHON_RUFF_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["python", "python_stub"],
    "context": "owned",
    "execution": "direct",
    "family": "python",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ruff",
    "transport": "batched",
}

PYTHON_TY_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["python", "python_stub"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "python",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "ty",
    "transport": "batched",
}

JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["jupyter"],
    "context": "owned",
    "execution": "direct",
    "family": "jupyter",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "notebook_outputs",
    "transport": "batched",
}

REGO_OPA_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["rego"],
    "context": "owned",
    "execution": "direct",
    "family": "rego",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "opa",
    "transport": "single",
}

REGO_OPA_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["rego"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "rego",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "opa",
    "transport": "batched",
}

REGO_REGAL_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["rego"],
    "context": "owned",
    "execution": "direct",
    "family": "rego",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "regal",
    "transport": "batched",
}

# linux_arm64: ELIXIR_CELLS includes it (#4544); no runner-matrix case proves this tool here yet (#4441)
# macos_arm64: ELIXIR_CELLS includes it (#4544); no runner-matrix case proves this tool here yet (#4441)
# windows_arm64: no OTP Windows cell in erlang/toolchain/pins.bzl (#4544)
# windows_x86_64: no OTP Windows cell in erlang/toolchain/pins.bzl (#4544)
ELIXIR_CREDO_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["elixir"],
    "context": "owned",
    "execution": "direct",
    "family": "elixir",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "credo",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

# linux_arm64: ELIXIR_CELLS includes it (#4544); no runner-matrix case proves this tool here yet (#4441)
# macos_arm64: ELIXIR_CELLS includes it (#4544); no runner-matrix case proves this tool here yet (#4441)
# windows_arm64: no OTP Windows cell in erlang/toolchain/pins.bzl (#4544)
# windows_x86_64: no OTP Windows cell in erlang/toolchain/pins.bzl (#4544)
ELIXIR_MIX_FORMAT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["elixir"],
    "context": "owned",
    "execution": "direct",
    "family": "elixir",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "mix_format",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

# windows_arm64: no OTP Windows cell in erlang/toolchain/pins.bzl
# windows_x86_64: no OTP Windows cell in erlang/toolchain/pins.bzl
ERLANG_ELVIS_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["erlang"],
    "context": "owned",
    "execution": "direct",
    "family": "erlang",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "elvis",
    "transport": "batched",
    "unsupported": ["windows_arm64", "windows_x86_64"],
}

# windows_arm64: no OTP Windows cell in erlang/toolchain/pins.bzl
# windows_x86_64: no OTP Windows cell in erlang/toolchain/pins.bzl
ERLANG_ERLFMT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["erlang"],
    "context": "owned",
    "execution": "direct",
    "family": "erlang",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "erlfmt",
    "transport": "batched",
    "unsupported": ["windows_arm64", "windows_x86_64"],
}

# windows_arm64: Julia ships no windows_arm64 runtime (julia/rules/platforms.bzl)
JULIA_JULIA_FORMAT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["julia"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "julia",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "julia_format",
    "transport": "single",
    "unsupported": ["windows_arm64"],
}

# linux_arm64: LDC_CELLS includes it; no runner-matrix case proves this tool here yet (#4664)
# macos_arm64: LDC_CELLS includes it; no runner-matrix case proves this tool here yet (#4664)
# windows_arm64: LDC_CELLS in d/toolchain/pins.bzl has no windows cell
# windows_x86_64: LDC_CELLS in d/toolchain/pins.bzl has no windows cell
D_DFMT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["d"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "d",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "dfmt",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

# linux_arm64: LDC_CELLS includes it; no runner-matrix case proves this tool here yet (#4664)
# macos_arm64: LDC_CELLS includes it; no runner-matrix case proves this tool here yet (#4664)
# windows_arm64: LDC_CELLS in d/toolchain/pins.bzl has no windows cell
# windows_x86_64: LDC_CELLS in d/toolchain/pins.bzl has no windows cell
D_DSCANNER_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["d"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "d",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "dscanner",
    "transport": "batched",
    "unsupported": ["linux_arm64", "macos_arm64", "windows_arm64", "windows_x86_64"],
}

ZIG_ZIG_FMT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["zig"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "zig",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "zig_fmt",
    "transport": "single",
    "unsupported": [],
}

# windows_arm64: rules_dart ships no windows_arm64 SDK (dart/rules/platforms.bzl)
DART_DART_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["dart"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "dart",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "dart",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

# windows_arm64: rules_dart ships no windows_arm64 SDK (dart/rules/platforms.bzl)
DART_DART_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["dart"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "dart",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "dart",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

# windows_arm64: rules_dart ships no windows_arm64 SDK (dart/rules/platforms.bzl)
DART_DART_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["dart"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "dart",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "dart",
    "transport": "batched",
    "unsupported": ["windows_arm64"],
}

# windows_arm64: no Qt SDK cell in qt/sdk/pins.bzl
# windows_x86_64: Qt windows_x86_64 cell has a blocker in qt/sdk/pins.bzl
QML_QMLLINT_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["qml"],
    "context": "owned",
    "execution": "direct",
    "family": "qml",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "qmllint",
    "transport": "batched",
    "unsupported": ["windows_arm64", "windows_x86_64"],
}

# windows_arm64: no Qt SDK cell in qt/sdk/pins.bzl
# windows_x86_64: Qt windows_x86_64 cell has a blocker in qt/sdk/pins.bzl
QML_QMLLINT_TYPECHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "typecheck",
    "classes": ["qml"],
    "context": "owned",
    "execution": "direct",
    "family": "qml",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "qmllint",
    "transport": "batched",
    "unsupported": ["windows_arm64", "windows_x86_64"],
}

# windows_arm64: no Qt SDK cell in qt/sdk/pins.bzl
# windows_x86_64: Qt windows_x86_64 cell has a blocker in qt/sdk/pins.bzl
QML_QMLFORMAT_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["qml"],
    "context": "owned",
    "execution": "direct",
    "family": "qml",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64"],
    "state": "supported",
    "tool": "qmlformat",
    "transport": "single",
    "unsupported": ["windows_arm64", "windows_x86_64"],
}

SUPPORT_RECORDS = [
    CAPNP_CAPNP_LINT_SUPPORT,
    CAPNP_CAPNP_TYPECHECK_SUPPORT,
    FLATBUFFERS_FLATC_LINT_SUPPORT,
    FLATBUFFERS_FLATC_TYPECHECK_SUPPORT,
    THRIFT_THRIFT_LINT_SUPPORT,
    THRIFT_THRIFT_TYPECHECK_SUPPORT,
    SCALA_SCALAFMT_SUPPORT,
    SCALA_SCALAC_SUPPORT,
    SCALA_SCALAFIX_SUPPORT,
    JAVA_CHECKSTYLE_SUPPORT,
    JAVA_GOOGLE_JAVA_FORMAT_SUPPORT,
    JAVA_JAVAC_SUPPORT,
    JAVA_PMD_SUPPORT,
    JAVA_SPOTBUGS_SUPPORT,
    KOTLIN_KOTLINC_SUPPORT,
    KOTLIN_KTFMT_SUPPORT,
    KOTLIN_KTLINT_SUPPORT,
    CSHARP_CSHARPIER_SUPPORT,
    CSHARP_CSC_SUPPORT,
    CSHARP_ROSLYN_SUPPORT,
    FSHARP_FANTOMAS_SUPPORT,
    FSHARP_FSC_SUPPORT,
    FSHARP_FSHARPLINT_SUPPORT,
    RUBY_RUBOCOP_SUPPORT,
    RUBY_STANDARDRB_SUPPORT,
    YAML_YAMLFMT_SUPPORT,
    YAML_YAMLLINT_SUPPORT,
    NIX_STATIX_SUPPORT,
    NIX_DEADNIX_SUPPORT,
    NIX_NIXFMT_SUPPORT,
    OCAML_TOPIARY_FORMAT_SUPPORT,
    OCAML_ZANUDA_LINT_SUPPORT,
    HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT,
    HTML_TEMPLATE_DJLINT_LINT_SUPPORT,
    TEXT_KEEP_SORTED_SUPPORT,
    SHELL_SHELLCHECK_SUPPORT,
    SHELL_SHFMT_SUPPORT,
    DOCKERFILE_HADOLINT_SUPPORT,
    KUBERNETES_KUBECONFORM_SUPPORT,
    HELM_HELM_SUPPORT,
    ANSIBLE_ANSIBLE_LINT_SUPPORT,
    BICEP_BICEP_FORMAT_SUPPORT,
    BICEP_BICEP_LINT_SUPPORT,
    CLOUDFORMATION_CFN_LINT_SUPPORT,
    MAKE_CHECKMAKE_SUPPORT,
    MAKE_MBAKE_SUPPORT,
    CMAKE_GERSEMI_SUPPORT,
    CMAKE_CMAKELINT_SUPPORT,
    TERRAFORM_FORMAT_SUPPORT,
    TERRAFORM_TFLINT_SUPPORT,
    GO_GOFUMPT_FORMAT_SUPPORT,
    GO_ERRCHECK_LINT_SUPPORT,
    GO_GOVET_LINT_SUPPORT,
    GO_STATICCHECK_LINT_SUPPORT,
    CC_CLANG_FORMAT_SUPPORT,
    CUDA_CLANG_FORMAT_SUPPORT,
    OBJC_CLANG_FORMAT_SUPPORT,
    CC_CPPCHECK_LINT_SUPPORT,
    CC_CLANG_TIDY_LINT_SUPPORT,
    RUST_RUSTFMT_FORMAT_SUPPORT,
    RUST_CLIPPY_LINT_SUPPORT,
    RUST_RUSTC_TYPECHECK_SUPPORT,
    PROTOBUF_BUF_FORMAT_SUPPORT,
    PROTOBUF_BUF_LINT_SUPPORT,
    CSS_STYLELINT_LINT_SUPPORT,
    GRAPHQL_ESLINT_LINT_SUPPORT,
    API_SPEC_REDOCLY_LINT_SUPPORT,
    MDX_REMARK_FORMAT_SUPPORT,
    API_SPEC_PRETTIER_FORMAT_SUPPORT,
    ANSIBLE_PRETTIER_FORMAT_SUPPORT,
    KUBERNETES_PRETTIER_FORMAT_SUPPORT,
    DOCKERFILE_PRETTIER_FORMAT_SUPPORT,
    PERL_PERLCRITIC_LINT_SUPPORT,
    PERL_PERLTIDY_FORMAT_SUPPORT,
    PHP_PHPCS_LINT_SUPPORT,
    PHP_PHPSTAN_TYPECHECK_SUPPORT,
    PHP_PRETTIER_FORMAT_SUPPORT,
    SQL_PRETTIER_FORMAT_SUPPORT,
    SQL_SQLFLUFF_FORMAT_SUPPORT,
    SQL_SQLFLUFF_LINT_SUPPORT,
    XML_PRETTIER_FORMAT_SUPPORT,
    GHERKIN_PRETTIER_FORMAT_SUPPORT,
    GHERKIN_GPLINT_LINT_SUPPORT,
    CUE_FORMAT_SUPPORT,
    CUE_TYPECHECK_SUPPORT,
    JSONNET_FORMAT_SUPPORT,
    JSONNET_LINT_SUPPORT,
    FORTRAN_FFMT_FORMAT_SUPPORT,
    FORTRAN_FORTITUDE_LINT_SUPPORT,
    XML_XMLLINT_LINT_SUPPORT,
    HASKELL_HLINT_LINT_SUPPORT,
    HASKELL_ORMOLU_FORMAT_SUPPORT,
    TYPST_TYPSTYLE_FORMAT_SUPPORT,
    LATEX_LATEXINDENT_FORMAT_SUPPORT,
    LATEX_CHKTEX_LINT_SUPPORT,
    ASCIIDOC_VALE_LINT_SUPPORT,
    PKL_FORMAT_SUPPORT,
    PKL_TYPECHECK_SUPPORT,
    LUA_SELENE_LINT_SUPPORT,
    LUA_STYLUA_SUPPORT,
    REGO_OPA_FORMAT_SUPPORT,
    REGO_OPA_TYPECHECK_SUPPORT,
    REGO_REGAL_LINT_SUPPORT,
    POWERSHELL_PSSCRIPTANALYZER_FORMAT_SUPPORT,
    POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT,
    PYTHON_FLAKE8_LINT_SUPPORT,
    PYTHON_PYDOCLINT_LINT_SUPPORT,
    PYTHON_PYLINT_LINT_SUPPORT,
    PYTHON_RUFF_FORMAT_SUPPORT,
    PYTHON_RUFF_LINT_SUPPORT,
    PYTHON_TY_TYPECHECK_SUPPORT,
    JUPYTER_RUFF_FORMAT_SUPPORT,
    JUPYTER_RUFF_LINT_SUPPORT,
    JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT,
    JAVASCRIPT_BIOME_FORMAT_SUPPORT,
    JAVASCRIPT_BIOME_LINT_SUPPORT,
    JAVASCRIPT_ESLINT_LINT_SUPPORT,
    JAVASCRIPT_PRETTIER_FORMAT_SUPPORT,
    ASTRO_ESLINT_LINT_SUPPORT,
    SVELTE_ESLINT_LINT_SUPPORT,
    VUE_ESLINT_LINT_SUPPORT,
    MDX_ESLINT_LINT_SUPPORT,
    TYPESCRIPT_BIOME_FORMAT_SUPPORT,
    TYPESCRIPT_BIOME_LINT_SUPPORT,
    D_DFMT_FORMAT_SUPPORT,
    D_DSCANNER_LINT_SUPPORT,
    ELIXIR_CREDO_LINT_SUPPORT,
    ELIXIR_MIX_FORMAT_FORMAT_SUPPORT,
    ERLANG_ELVIS_LINT_SUPPORT,
    ERLANG_ERLFMT_FORMAT_SUPPORT,
    JULIA_JULIA_FORMAT_FORMAT_SUPPORT,
    ZIG_ZIG_FMT_FORMAT_SUPPORT,
    DART_DART_LINT_SUPPORT,
    DART_DART_FORMAT_SUPPORT,
    DART_DART_TYPECHECK_SUPPORT,
    QML_QMLLINT_LINT_SUPPORT,
    QML_QMLLINT_TYPECHECK_SUPPORT,
    QML_QMLFORMAT_FORMAT_SUPPORT,
]

def _is_canonical_token(text):
    """Reports whether a token uses the canonical alphabet."""
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

# Every platform a support record may name: the qualified execution set plus the selectable
# fallback platforms (hub.bzl). A sixth platform is one edit there.
# A fallback platform claim still needs a matrix case flagged for it; fallbacks supply artifacts only.
SUPPORT_PLATFORMS = TOOL_PLATFORMS + sorted(PLATFORM_FALLBACKS.keys())

def _is_unbuilt(record):
    """Reports whether a record names a tool with no pinned build on any platform.

    Only a known record with a missing artifact may leave `platforms` empty, and it must list
    every platform as unsupported.
    """
    unsupported = record.get("unsupported", [])
    return (
        record["artifact"] == "missing" and
        record["state"] == "known" and
        type(unsupported) == "list" and
        sorted(unsupported) == sorted(SUPPORT_PLATFORMS)
    )

def support_unpinned_tools(capability):
    """Maps each tool of one capability with no pinned build anywhere to its unsupported platforms."""
    unpinned = {}
    for record in SUPPORT_RECORDS:
        if record["capability"] == capability and len(record["platforms"]) == 0 and _is_unbuilt(record):
            unpinned[record["tool"]] = sorted(record["unsupported"])
    return unpinned

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
    if type(platforms) != "list":
        return "support: tool '" + tool + "' needs a non-empty platform list"
    if len(platforms) == 0 and not _is_unbuilt(record):
        return "support: tool '" + tool + "' needs a non-empty platform list"
    for platform in platforms:
        if not _is_canonical_token(platform):
            return "support: non-canonical platform '" + str(platform) + "' for tool '" + tool + "'"
        if platform not in SUPPORT_PLATFORMS:
            return "support: unknown platform '" + str(platform) + "' for tool '" + tool + "'"
    unsupported = record.get("unsupported", [])
    if type(unsupported) != "list":
        return "support: tool '" + tool + "' needs 'unsupported' to be a list"
    for platform in unsupported:
        if platform not in SUPPORT_PLATFORMS:
            return "support: unknown unsupported platform '" + str(platform) + "' for tool '" + tool + "'"
        if platform in platforms:
            return "support: platform '" + platform + "' is both supported and unsupported for tool '" + tool + "'"
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

def support_record_gap_error(record, wired):
    """Reports the first wiring or pin gap for one supported or qualified record."""
    state = record["state"]
    if state not in ["qualified", "supported"]:
        return ""
    prefix = "support: " + record["tool"] + " " + record["capability"] + " is " + state
    if record["capability"] not in wired.get(record["tool"], []):
        return prefix + " but has no wired aspect (not silently clean)"
    if record["artifact"] != "pinned":
        return prefix + " but has no pinned artifact"
    return ""

def support_gap_error(wired):
    """Returns the first wiring or pin gap across every support record."""
    for record in SUPPORT_RECORDS:
        err = support_record_gap_error(record, wired)
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
