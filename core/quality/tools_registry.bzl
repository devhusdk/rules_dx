"""Generated from //tools/quality:family_tools.bzl by //core/quality:tools_registry; refresh with bazel run //core/quality:tools_registry."""

FAMILY_TOOL_ROWS = json.decode(r"""{
  "ansible": [
    {
      "adapters": {
        "lint": [
          "ansible"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".ansible-lint",
          ".yml",
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "ansible-lint moderate and production profiles"
      },
      "id": "ansible_lint",
      "severity": {
        "categories": {
          "internal-error": "error",
          "load-failure": "error",
          "syntax-check": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "ansible"
          ],
          "context": "owned",
          "family": "ansible",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "ansible_lint",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "ansible-core does not run on Windows",
            "windows_x86_64": "ansible-core does not run on Windows"
          }
        }
      ]
    }
  ],
  "beam": [
    {
      "adapters": {
        "lint": [
          "elixir"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".exs"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "credo",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "elixir"
          ],
          "context": "owned",
          "family": "elixir",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "credo",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "erlang",
          "elixir"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "dialyzer",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "erlang"
          ],
          "context": "target_coupled",
          "family": "erlang",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "dialyzer",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "elixir"
          ],
          "context": "target_coupled",
          "family": "elixir",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "dialyzer",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "erlang"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          "elvis.config",
          ".config"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "elvis",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "erlang"
          ],
          "context": "owned",
          "family": "erlang",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "elvis",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "elixir"
        ]
      },
      "concerns": {
        "typecheck": "unused_code"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "elixirc",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "elixir"
          ],
          "context": "target_coupled",
          "family": "elixir",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "elixirc",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "The elixirc replay needs a POSIX shell",
            "windows_x86_64": "The elixirc replay needs a POSIX shell"
          }
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "erlang"
        ]
      },
      "concerns": {
        "typecheck": "unused_code"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "erlc",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "erlang"
          ],
          "context": "target_coupled",
          "family": "erlang",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "erlc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "erlang"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "erlfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "erlang"
          ],
          "context": "owned",
          "family": "erlang",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "erlfmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "elixir"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "#",
        "ext": "exs",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".formatter.exs"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".exs"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "mix_format",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "elixir"
          ],
          "context": "owned",
          "family": "elixir",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "mix_format",
          "transport": "batched"
        }
      ]
    }
  ],
  "bicep": [
    {
      "adapters": {
        "format": [
          "bicep"
        ],
        "lint": [
          "bicep"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "bicepconfig.json"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          "bicepconfig.json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "bicep",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "bicep"
          ],
          "context": "owned",
          "family": "bicep",
          "fix": "supported",
          "state": "supported",
          "tool": "bicep",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "bicep"
          ],
          "context": "target_coupled",
          "family": "bicep",
          "fix": "check_only",
          "state": "supported",
          "tool": "bicep",
          "transport": "single"
        }
      ]
    }
  ],
  "cc": [
    {
      "adapters": {
        "format": [
          "c",
          "cpp",
          "cuda",
          "objc",
          "objcpp"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".clang-format"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".clang-format"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "clang_format",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "c",
            "cpp"
          ],
          "context": "owned",
          "family": "cc",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clang_format",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "cuda"
          ],
          "context": "owned",
          "family": "cuda",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clang_format",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "objc",
            "objcpp"
          ],
          "context": "owned",
          "family": "objc",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clang_format",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "c",
          "cpp",
          "objc",
          "objcpp"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "clang_syntax",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "c",
            "cpp"
          ],
          "context": "target_coupled",
          "family": "cc",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clang_syntax",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "objc",
            "objcpp"
          ],
          "context": "target_coupled",
          "family": "objc",
          "fix": "check_only",
          "platforms": [
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "clang_syntax",
          "transport": "batched",
          "unsupported": {
            "linux_arm64": "objc_library builds only with an Apple toolchain",
            "linux_x86_64": "objc_library builds only with an Apple toolchain",
            "windows_arm64": "objc_library builds only with an Apple toolchain",
            "windows_x86_64": "objc_library builds only with an Apple toolchain"
          }
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "c",
          "cpp",
          "cuda",
          "objc",
          "objcpp"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".clang-tidy"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "clang_tidy",
      "severity": {
        "categories": {
          "bugprone-": "warning",
          "cert-": "warning",
          "clang-analyzer-": "warning",
          "clang-diagnostic-error": "error",
          "cppcoreguidelines-": "warning",
          "misc-": "warning",
          "modernize-": "warning",
          "performance-": "warning",
          "readability-": "warning"
        },
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "c",
            "cpp"
          ],
          "context": "target_coupled",
          "family": "cc",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clang_tidy",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "cuda"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "nvcc_syntax",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "cuda"
          ],
          "context": "target_coupled",
          "family": "cuda",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "nvcc_syntax",
          "transport": "batched"
        }
      ]
    }
  ],
  "cloudformation": [
    {
      "adapters": {
        "lint": [
          "cloudformation"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".cfnlintrc",
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "cfn_lint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "cloudformation"
          ],
          "context": "owned",
          "family": "cloudformation",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "cfn_lint",
          "transport": "batched"
        }
      ]
    }
  ],
  "cmake": [
    {
      "adapters": {
        "lint": [
          "cmake"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "cmakelintrc",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".cmakelintrc"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".cmakelintrc"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "cmakelint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "cmake"
          ],
          "context": "owned",
          "family": "cmake",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "cmakelint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "cmake"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "yaml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".gersemirc"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".gersemirc",
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "gersemi",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "cmake"
          ],
          "context": "owned",
          "family": "cmake",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "gersemi",
          "transport": "batched"
        }
      ]
    }
  ],
  "core": [
    {
      "adapters": {
        "lint": [
          "github_workflow"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "actionlint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "github_workflow"
          ],
          "context": "owned",
          "family": "core",
          "fix": "check_only",
          "state": "supported",
          "tool": "actionlint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "avro"
        ],
        "typecheck": [
          "avro"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "avro_tools",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "avro"
          ],
          "context": "owned",
          "family": "avro",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "avro_tools",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "avro"
          ],
          "context": "owned",
          "family": "avro",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "avro_tools",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "protobuf"
        ],
        "lint": [
          "protobuf"
        ],
        "typecheck": [
          "protobuf"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "yaml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "buf.yaml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "buf",
      "severity": {
        "categories": {
          "COMPILE": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "protobuf"
          ],
          "context": "owned",
          "family": "protobuf",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "buf",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "protobuf"
          ],
          "context": "owned",
          "family": "protobuf",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "buf",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "protobuf"
          ],
          "context": "owned",
          "family": "protobuf",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "buf",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "starlark"
        ],
        "lint": [
          "starlark"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "json",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".buildifier.json"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "buildifier",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "starlark"
          ],
          "context": "owned",
          "family": "starlark",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "buildifier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "starlark"
          ],
          "context": "owned",
          "family": "starlark",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "buildifier",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "capnp"
        ],
        "typecheck": [
          "capnp"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "capnp",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "capnp"
          ],
          "context": "owned",
          "family": "capnp",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "capnp",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "capnp"
          ],
          "context": "owned",
          "family": "capnp",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "capnp",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "flatbuffers"
        ],
        "typecheck": [
          "flatbuffers"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "flatc",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "flatbuffers"
          ],
          "context": "owned",
          "family": "flatbuffers",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "flatc",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "flatbuffers"
          ],
          "context": "owned",
          "family": "flatbuffers",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "flatc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "gherkin"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".gplintrc",
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "gplint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "gherkin"
          ],
          "context": "owned",
          "family": "gherkin",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "gplint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "graphql"
        ],
        "typecheck": [
          "graphql"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "graphql_eslint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "graphql"
          ],
          "context": "owned",
          "family": "graphql",
          "fix": "check_only",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "graphql_eslint",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "graphql"
          ],
          "context": "owned",
          "family": "graphql",
          "fix": "check_only",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "graphql_eslint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "asciidoc",
          "html",
          "markdown",
          "mdx",
          "rst"
        ]
      },
      "concerns": {
        "lint": "links"
      },
      "config": {
        "comment": "#",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "lychee.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "lychee",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "asciidoc"
          ],
          "context": "owned",
          "family": "asciidoc",
          "fix": "check_only",
          "state": "supported",
          "tool": "lychee",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "html"
          ],
          "context": "owned",
          "family": "html",
          "fix": "check_only",
          "state": "supported",
          "tool": "lychee",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "markdown"
          ],
          "context": "owned",
          "family": "markdown",
          "fix": "check_only",
          "state": "supported",
          "tool": "lychee",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "mdx"
          ],
          "context": "owned",
          "family": "mdx",
          "fix": "check_only",
          "state": "supported",
          "tool": "lychee",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "rst"
          ],
          "context": "owned",
          "family": "rst",
          "fix": "check_only",
          "state": "supported",
          "tool": "lychee",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "markdown"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "markdown_check",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": []
    },
    {
      "adapters": {
        "format": [
          "api_spec",
          "api_spec_ref",
          "astro",
          "avro",
          "cloudformation",
          "css",
          "dockerfile",
          "gherkin",
          "graphql",
          "helm",
          "html",
          "json5",
          "less",
          "markdown",
          "php",
          "scss",
          "svelte",
          "vue",
          "xml"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".prettierrc",
          ".json",
          ".json5",
          ".yaml",
          ".yml",
          ".toml",
          ".mjs",
          ".cjs",
          ".js"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "prettier",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "api_spec",
            "api_spec_ref"
          ],
          "context": "owned",
          "family": "api_spec",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "dockerfile"
          ],
          "context": "owned",
          "family": "dockerfile",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "php"
          ],
          "context": "owned",
          "family": "php",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "xml"
          ],
          "context": "owned",
          "family": "xml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "gherkin"
          ],
          "context": "owned",
          "family": "gherkin",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "avro"
          ],
          "context": "owned",
          "family": "avro",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "cloudformation"
          ],
          "context": "owned",
          "family": "cloudformation",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "markdown"
          ],
          "context": "owned",
          "family": "markdown",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "helm"
          ],
          "context": "owned",
          "family": "helm",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "prettier",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "qml"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".qmlformat.ini"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "qmlformat",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "qml"
          ],
          "context": "owned",
          "family": "qml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "qmlformat",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "qml"
        ],
        "typecheck": [
          "qml"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".qmllint.ini"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "qmllint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "qml"
          ],
          "context": "owned",
          "family": "qml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "qmllint",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "qml"
          ],
          "context": "owned",
          "family": "qml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "qmllint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "api_spec"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "redocly spec and all rulesets"
      },
      "id": "redocly",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "api_spec"
          ],
          "context": "owned",
          "family": "api_spec",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "redocly",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "toml"
        ],
        "lint": [
          "toml"
        ],
        "typecheck": [
          "toml"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".taplo.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "taplo",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "toml"
          ],
          "context": "owned",
          "family": "toml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "taplo",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "toml"
          ],
          "context": "owned",
          "family": "toml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "taplo",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "toml"
          ],
          "context": "owned",
          "family": "toml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "taplo",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "thrift"
        ],
        "typecheck": [
          "thrift"
        ]
      },
      "concerns": {
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "thrift",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "thrift"
          ],
          "context": "owned",
          "family": "thrift",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "thrift",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "thrift"
          ],
          "context": "owned",
          "family": "thrift",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "thrift",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "bicep",
          "cloudformation",
          "helm",
          "kubernetes",
          "terraform"
        ]
      },
      "concerns": {
        "lint": "security"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "trivy_config",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "bicep"
          ],
          "context": "owned",
          "family": "bicep",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_config",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "cloudformation"
          ],
          "context": "owned",
          "family": "cloudformation",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_config",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "helm"
          ],
          "context": "owned",
          "family": "helm",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_config",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "kubernetes"
          ],
          "context": "owned",
          "family": "kubernetes",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_config",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "terraform"
          ],
          "context": "owned",
          "family": "terraform",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_config",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "ansible",
          "api_spec",
          "asciidoc",
          "astro",
          "avro",
          "bicep",
          "c",
          "capnp",
          "cloudformation",
          "cmake",
          "cpp",
          "csharp",
          "css",
          "cuda",
          "d",
          "dart",
          "dockerfile",
          "elixir",
          "erlang",
          "flatbuffers",
          "fortran",
          "fsharp",
          "gherkin",
          "go",
          "go_module",
          "graphql",
          "haskell",
          "helm",
          "html",
          "html_template",
          "java",
          "javascript",
          "jsonnet",
          "jsx",
          "julia",
          "kotlin",
          "kubernetes",
          "latex",
          "less",
          "lua",
          "make",
          "markdown",
          "mdx",
          "nix",
          "objc",
          "objcpp",
          "ocaml",
          "perl",
          "php",
          "pkl",
          "powershell",
          "protobuf",
          "python",
          "python_stub",
          "qml",
          "rego",
          "rst",
          "ruby",
          "rust",
          "scala",
          "scss",
          "shell",
          "sql",
          "starlark",
          "svelte",
          "swift",
          "terraform",
          "text",
          "thrift",
          "toml",
          "tsx",
          "typescript",
          "typst",
          "vue",
          "xml",
          "yaml",
          "zig"
        ]
      },
      "concerns": {
        "lint": "spelling"
      },
      "config": {
        "comment": "#",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "typos.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "typos",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "ansible"
          ],
          "context": "owned",
          "family": "ansible",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "api_spec"
          ],
          "context": "owned",
          "family": "api_spec",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "asciidoc"
          ],
          "context": "owned",
          "family": "asciidoc",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "astro"
          ],
          "context": "owned",
          "family": "astro",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "avro"
          ],
          "context": "owned",
          "family": "avro",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "bicep"
          ],
          "context": "owned",
          "family": "bicep",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "capnp"
          ],
          "context": "owned",
          "family": "capnp",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "c",
            "cpp"
          ],
          "context": "owned",
          "family": "cc",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "cloudformation"
          ],
          "context": "owned",
          "family": "cloudformation",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "cmake"
          ],
          "context": "owned",
          "family": "cmake",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "csharp"
          ],
          "context": "owned",
          "family": "csharp",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "css",
            "less",
            "scss"
          ],
          "context": "owned",
          "family": "css",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "cuda"
          ],
          "context": "owned",
          "family": "cuda",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "d"
          ],
          "context": "owned",
          "family": "d",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "dart"
          ],
          "context": "owned",
          "family": "dart",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "dockerfile"
          ],
          "context": "owned",
          "family": "dockerfile",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "elixir"
          ],
          "context": "owned",
          "family": "elixir",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "erlang"
          ],
          "context": "owned",
          "family": "erlang",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "flatbuffers"
          ],
          "context": "owned",
          "family": "flatbuffers",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "fortran"
          ],
          "context": "owned",
          "family": "fortran",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "fsharp"
          ],
          "context": "owned",
          "family": "fsharp",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "gherkin"
          ],
          "context": "owned",
          "family": "gherkin",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "go"
          ],
          "context": "owned",
          "family": "go",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "go_module"
          ],
          "context": "owned",
          "family": "go_module",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "graphql"
          ],
          "context": "owned",
          "family": "graphql",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "haskell"
          ],
          "context": "owned",
          "family": "haskell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "helm"
          ],
          "context": "owned",
          "family": "helm",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "html"
          ],
          "context": "owned",
          "family": "html",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "html_template"
          ],
          "context": "owned",
          "family": "html_template",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "java"
          ],
          "context": "owned",
          "family": "java",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "javascript",
            "jsx"
          ],
          "context": "owned",
          "family": "javascript",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "jsonnet"
          ],
          "context": "owned",
          "family": "jsonnet",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "julia"
          ],
          "context": "owned",
          "family": "julia",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "kotlin"
          ],
          "context": "owned",
          "family": "kotlin",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "kubernetes"
          ],
          "context": "owned",
          "family": "kubernetes",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "latex"
          ],
          "context": "owned",
          "family": "latex",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "lua"
          ],
          "context": "owned",
          "family": "lua",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "make"
          ],
          "context": "owned",
          "family": "make",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "markdown"
          ],
          "context": "owned",
          "family": "markdown",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "mdx"
          ],
          "context": "owned",
          "family": "mdx",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "nix"
          ],
          "context": "owned",
          "family": "nix",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "objc",
            "objcpp"
          ],
          "context": "owned",
          "family": "objc",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "ocaml"
          ],
          "context": "owned",
          "family": "ocaml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "perl"
          ],
          "context": "owned",
          "family": "perl",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "php"
          ],
          "context": "owned",
          "family": "php",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "pkl"
          ],
          "context": "owned",
          "family": "pkl",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "powershell"
          ],
          "context": "owned",
          "family": "powershell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "protobuf"
          ],
          "context": "owned",
          "family": "protobuf",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "python",
            "python_stub"
          ],
          "context": "owned",
          "family": "python",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "qml"
          ],
          "context": "owned",
          "family": "qml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "rego"
          ],
          "context": "owned",
          "family": "rego",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "rst"
          ],
          "context": "owned",
          "family": "rst",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "ruby"
          ],
          "context": "owned",
          "family": "ruby",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "rust"
          ],
          "context": "owned",
          "family": "rust",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "scala"
          ],
          "context": "owned",
          "family": "scala",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "shell"
          ],
          "context": "owned",
          "family": "shell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "sql"
          ],
          "context": "owned",
          "family": "sql",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "starlark"
          ],
          "context": "owned",
          "family": "starlark",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "svelte"
          ],
          "context": "owned",
          "family": "svelte",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "swift"
          ],
          "context": "owned",
          "family": "swift",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "terraform"
          ],
          "context": "owned",
          "family": "terraform",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "text"
          ],
          "context": "owned",
          "family": "text",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "thrift"
          ],
          "context": "owned",
          "family": "thrift",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "toml"
          ],
          "context": "owned",
          "family": "toml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "tsx",
            "typescript"
          ],
          "context": "owned",
          "family": "typescript",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "typst"
          ],
          "context": "owned",
          "family": "typst",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "vue"
          ],
          "context": "owned",
          "family": "vue",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "xml"
          ],
          "context": "owned",
          "family": "xml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "yaml"
          ],
          "context": "owned",
          "family": "yaml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "zig"
          ],
          "context": "owned",
          "family": "zig",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "typos",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "github_workflow"
        ]
      },
      "concerns": {
        "lint": "security"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "zizmor",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "github_workflow"
          ],
          "context": "owned",
          "family": "core",
          "fix": "check_only",
          "state": "supported",
          "tool": "zizmor",
          "transport": "single"
        }
      ]
    }
  ],
  "cue": [
    {
      "adapters": {
        "format": [
          "cue"
        ],
        "typecheck": [
          "cue"
        ]
      },
      "concerns": {
        "format": "format",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "cue",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "cue"
          ],
          "context": "owned",
          "family": "cue",
          "fix": "supported",
          "state": "qualified",
          "tool": "cue",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "cue"
          ],
          "context": "target_coupled",
          "family": "cue",
          "fix": "check_only",
          "state": "qualified",
          "tool": "cue",
          "transport": "batched"
        }
      ]
    }
  ],
  "d": [
    {
      "adapters": {
        "typecheck": [
          "d"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ldc",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    }
  ],
  "dart": [
    {
      "adapters": {
        "format": [
          "dart"
        ],
        "lint": [
          "dart"
        ],
        "typecheck": [
          "dart"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "#",
        "ext": "yaml",
        "extends_syntax": "include: {base}\n",
        "extensions": [],
        "extras": [],
        "family": "dart",
        "files": [
          "analysis_options.yaml"
        ],
        "init_order": 5,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "dart",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "dart"
          ],
          "context": "target_coupled",
          "family": "dart",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "dart",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "rules_dart ships no windows_arm64 SDK"
          }
        },
        {
          "capability": "format",
          "classes": [
            "dart"
          ],
          "context": "target_coupled",
          "family": "dart",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "dart",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "rules_dart ships no windows_arm64 SDK"
          }
        },
        {
          "capability": "typecheck",
          "classes": [
            "dart"
          ],
          "context": "target_coupled",
          "family": "dart",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "dart",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "rules_dart ships no windows_arm64 SDK"
          }
        }
      ]
    }
  ],
  "dockerfile": [
    {
      "adapters": {
        "lint": [
          "dockerfile"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "hadolint",
      "severity": {
        "categories": {
          "DL1": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "dockerfile"
          ],
          "context": "owned",
          "family": "dockerfile",
          "fix": "check_only",
          "state": "supported",
          "tool": "hadolint",
          "transport": "batched"
        }
      ]
    }
  ],
  "dotnet": [
    {
      "adapters": {
        "typecheck": [
          "csharp"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "csc",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "csharp"
          ],
          "context": "target_coupled",
          "family": "csharp",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "csc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "csharp"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "csharpier",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "csharp"
          ],
          "context": "owned",
          "family": "csharp",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "csharpier",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "fsharp"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "fantomas",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "fsharp"
          ],
          "context": "owned",
          "family": "fsharp",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "fantomas",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "fsharp"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "fsc",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "fsharp"
          ],
          "context": "target_coupled",
          "family": "fsharp",
          "fix": "check_only",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "fsc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "fsharp"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "fsharplint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "fsharp"
          ],
          "context": "target_coupled",
          "family": "fsharp",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "fsharplint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "csharp"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".globalconfig"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "roslyn",
      "severity": {
        "categories": {
          "CA2100": "error",
          "CA2109": "error",
          "CA2119": "error",
          "CA2153": "error",
          "CA23": "error",
          "CA3": "error",
          "CA5": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "csharp"
          ],
          "context": "target_coupled",
          "family": "csharp",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "roslyn",
          "transport": "batched"
        }
      ]
    }
  ],
  "fortran": [
    {
      "adapters": {
        "format": [
          "fortran"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "ffmt.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ffmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "fortran"
          ],
          "context": "owned",
          "family": "fortran",
          "fix": "supported",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "ffmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "fortran"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "flang",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "lint": [
          "fortran"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "fortitude",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "fortran"
          ],
          "context": "owned",
          "family": "fortran",
          "fix": "check_only",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "fortitude",
          "transport": "batched"
        }
      ]
    }
  ],
  "go": [
    {
      "adapters": {
        "lint": [
          "go"
        ]
      },
      "concerns": {
        "lint": "unchecked_errors"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "errcheck",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "go"
          ],
          "context": "target_coupled",
          "family": "go",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "errcheck",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "go"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "gofumpt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "go"
          ],
          "context": "owned",
          "family": "go",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "gofumpt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "go"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "gotype",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "go"
          ],
          "context": "target_coupled",
          "family": "go",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "gotype",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "go"
        ]
      },
      "concerns": {
        "lint": "vet"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "govet",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "go"
          ],
          "context": "target_coupled",
          "family": "go",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "govet",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "go_module"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "modfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "go_module"
          ],
          "context": "owned",
          "family": "go_module",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "modfmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "go"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "staticcheck.conf"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          ".conf"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "staticcheck",
      "severity": {
        "categories": {
          "QF": "info",
          "S": "warning",
          "U1000": "warning"
        },
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "go"
          ],
          "context": "target_coupled",
          "family": "go",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "known",
          "tool": "staticcheck",
          "transport": "batched"
        }
      ]
    }
  ],
  "haskell": [
    {
      "adapters": {
        "typecheck": [
          "haskell"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ghc",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "lint": [
          "haskell"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "#",
        "ext": "yaml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "haskell",
        "files": [
          ".hlint.yaml"
        ],
        "init_order": 7,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "hlint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "haskell"
          ],
          "context": "owned",
          "family": "haskell",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "hlint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "haskell"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ormolu",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "haskell"
          ],
          "context": "owned",
          "family": "haskell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "ormolu",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "haskell"
        ]
      },
      "concerns": {
        "lint": "unused_code"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "weeder",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "haskell"
          ],
          "context": "target_coupled",
          "family": "haskell",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "weeder",
          "transport": "single",
          "unsupported": {
            "windows_arm64": "GHC 9.10.3 ships no Windows arm64 bindist"
          }
        }
      ]
    }
  ],
  "html_template": [
    {
      "adapters": {
        "format": [
          "html_template"
        ],
        "lint": [
          "html_template"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          "djlint.toml",
          ".djlintrc"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "djlint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "html_template"
          ],
          "context": "owned",
          "family": "html_template",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "djlint",
          "transport": "single"
        },
        {
          "capability": "lint",
          "classes": [
            "html_template"
          ],
          "context": "owned",
          "family": "html_template",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "djlint",
          "transport": "single"
        }
      ]
    }
  ],
  "js": [
    {
      "adapters": {
        "format": [
          "javascript",
          "json",
          "jsonc",
          "jsx",
          "typescript",
          "tsx"
        ],
        "lint": [
          "javascript",
          "json",
          "jsonc",
          "jsx",
          "typescript",
          "tsx"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "biome.json"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [
          "linter.rules"
        ],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [
          ".cjs",
          ".css",
          ".cts",
          ".js",
          ".json",
          ".jsonc",
          ".jsx",
          ".mjs",
          ".mts",
          ".ts",
          ".tsx"
        ],
        "suppressions": [
          "biome-ignore"
        ],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "biome",
      "severity": {
        "categories": {
          "lint/a11y": "warning",
          "lint/complexity": "warning",
          "lint/correctness": "error",
          "lint/performance": "warning",
          "lint/security": "error",
          "lint/style": "warning",
          "lint/suspicious": "warning"
        },
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "javascript",
            "jsx"
          ],
          "context": "owned",
          "family": "javascript",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "javascript",
            "jsx"
          ],
          "context": "owned",
          "family": "javascript",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "tsx",
            "typescript"
          ],
          "context": "owned",
          "family": "typescript",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "tsx",
            "typescript"
          ],
          "context": "owned",
          "family": "typescript",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "json",
            "jsonc"
          ],
          "context": "owned",
          "family": "json",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "json",
            "jsonc"
          ],
          "context": "owned",
          "family": "json",
          "fix": "supported",
          "state": "supported",
          "tool": "biome",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "astro",
          "javascript",
          "jsx",
          "mdx",
          "svelte",
          "tsx",
          "typescript",
          "vue"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "//",
        "ext": "mjs",
        "extends_syntax": "export { default } from \"./{base}\";\n",
        "extensions": [],
        "extras": [],
        "family": "js",
        "files": [
          "eslint.config.mjs",
          "eslint.config.js",
          "eslint.config.cjs",
          "eslint.config.ts",
          ".eslintrc*"
        ],
        "init_order": 2,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".js",
          ".mjs",
          ".cjs"
        ],
        "suppression_extensions": [
          ".astro",
          ".cjs",
          ".cts",
          ".js",
          ".jsx",
          ".mjs",
          ".mts",
          ".svelte",
          ".ts",
          ".tsx",
          ".vue"
        ],
        "suppressions": [
          "eslint-disable"
        ],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "eslint",
      "severity": {
        "categories": {
          "security/": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "javascript"
          ],
          "context": "owned",
          "family": "javascript",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "tsx",
            "typescript"
          ],
          "context": "owned",
          "family": "typescript",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "astro"
          ],
          "context": "owned",
          "family": "astro",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "svelte"
          ],
          "context": "owned",
          "family": "svelte",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "vue"
          ],
          "context": "owned",
          "family": "vue",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "mdx"
          ],
          "context": "owned",
          "family": "mdx",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "eslint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "mdx"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "remark",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "mdx"
          ],
          "context": "owned",
          "family": "mdx",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "remark",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "css",
          "less",
          "scss"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "json",
        "extends_syntax": "{ \"extends\": [\"./{base}\"] }\n",
        "extensions": [
          ".css"
        ],
        "extras": [],
        "family": "",
        "files": [
          ".stylelintrc.json",
          ".stylelintrc*",
          "stylelint.config.*"
        ],
        "init_order": 4,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [
          ".css",
          ".less",
          ".scss"
        ],
        "suppressions": [
          "stylelint-disable"
        ],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "stylelint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "css",
            "less",
            "scss"
          ],
          "context": "owned",
          "family": "css",
          "fix": "check_only",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "stylelint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "astro",
          "javascript",
          "jsx",
          "mdx",
          "svelte",
          "tsx",
          "typescript",
          "vue"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "json",
        "extends_syntax": "{ \"extends\": \"./{base}\" }\n",
        "extensions": [],
        "extras": [],
        "family": "js",
        "files": [
          "tsconfig.json"
        ],
        "init_order": 3,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "tsconfig",
        "tier_source": "dx"
      },
      "id": "tsc",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": []
    }
  ],
  "jsonnet": [
    {
      "adapters": {
        "lint": [
          "jsonnet"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "jsonnet_lint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "jsonnet"
          ],
          "context": "owned",
          "family": "jsonnet",
          "fix": "check_only",
          "state": "supported",
          "tool": "jsonnet_lint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "jsonnet"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "jsonnetfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "jsonnet"
          ],
          "context": "owned",
          "family": "jsonnet",
          "fix": "supported",
          "state": "qualified",
          "tool": "jsonnetfmt",
          "transport": "batched"
        }
      ]
    }
  ],
  "julia": [
    {
      "adapters": {
        "typecheck": [
          "julia"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "jet",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "format": [
          "julia"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "julia_format",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "julia"
          ],
          "context": "target_coupled",
          "family": "julia",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "julia_format",
          "transport": "single",
          "unsupported": {
            "windows_arm64": "Julia ships no windows_arm64 runtime"
          }
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "julia"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "julialint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "julia"
          ],
          "context": "owned",
          "family": "julia",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "julialint",
          "transport": "single",
          "unsupported": {
            "windows_arm64": "Julia ships no windows_arm64 runtime"
          }
        }
      ]
    }
  ],
  "jvm": [
    {
      "adapters": {
        "lint": [
          "kotlin"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yml",
          ".yaml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "detekt",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "kotlin"
          ],
          "context": "owned",
          "family": "kotlin",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "detekt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "java"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "google_java_format",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "java"
          ],
          "context": "owned",
          "family": "java",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "google_java_format",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "java"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "javac",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "java"
          ],
          "context": "target_coupled",
          "family": "java",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "javac",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "kotlin"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "kotlinc",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "kotlin"
          ],
          "context": "target_coupled",
          "family": "kotlin",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "kotlinc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "kotlin"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ktfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "kotlin"
          ],
          "context": "owned",
          "family": "kotlin",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "ktfmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "java"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".xml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "pmd",
      "severity": {
        "categories": {
          "HardCodedCryptoKey": "error",
          "InsecureCryptoIv": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "java"
          ],
          "context": "owned",
          "family": "java",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "pmd",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "scala"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "scalac",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "scala"
          ],
          "context": "target_coupled",
          "family": "scala",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "scalac",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "scala"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".conf"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "scalafix",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "scala"
          ],
          "context": "target_coupled",
          "family": "scala",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "scalafix",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "scala"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".scalafmt.conf"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".conf"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "scalafmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "scala"
          ],
          "context": "owned",
          "family": "scala",
          "fix": "supported",
          "state": "supported",
          "tool": "scalafmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "java"
        ]
      },
      "concerns": {
        "lint": "bytecode"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".xml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "spotbugs",
      "severity": {
        "categories": {
          "ASE_": "error",
          "DMI_CONSTANT_DB_PASSWORD": "error",
          "DMI_EMPTY_DB_PASSWORD": "error",
          "HRS_": "error",
          "PT_": "error",
          "UNS_": "error",
          "USBC_": "error",
          "USO_": "error",
          "XSS_": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "java"
          ],
          "context": "target_coupled",
          "family": "java",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "spotbugs",
          "transport": "batched"
        }
      ]
    }
  ],
  "kubernetes": [
    {
      "adapters": {
        "lint": [
          "helm"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "helm",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "helm"
          ],
          "context": "owned",
          "family": "helm",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "helm",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "helm"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".helmfmt"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".helmfmt",
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "helmfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "helm"
          ],
          "context": "owned",
          "family": "helm",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "helmfmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "kubernetes"
        ]
      },
      "concerns": {
        "lint": "schema"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "kubernetes.kubeconform.json"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          ".kubeconform.json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "kubeconform",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "kubernetes"
          ],
          "context": "owned",
          "family": "kubernetes",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "kubeconform",
          "transport": "batched"
        }
      ]
    }
  ],
  "latex": [
    {
      "adapters": {
        "lint": [
          "latex"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "chktex",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "latex"
          ],
          "context": "owned",
          "family": "latex",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "chktex",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "latex"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "latexindent",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "latex"
          ],
          "context": "owned",
          "family": "latex",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "latexindent",
          "transport": "single"
        }
      ]
    }
  ],
  "lua": [
    {
      "adapters": {
        "typecheck": [
          "lua"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "luals",
      "severity": {
        "categories": {
          "luals/assign-type-mismatch": "error",
          "luals/cast-type-mismatch": "error",
          "luals/error": "error",
          "luals/param-type-mismatch": "error",
          "luals/return-type-mismatch": "error"
        },
        "default": "warning"
      },
      "support": []
    },
    {
      "adapters": {
        "lint": [
          "lua"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "selene",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "lua"
          ],
          "context": "owned",
          "family": "lua",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "selene",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "the dx_quality_crates hub has no windows_arm64 cell"
          }
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "lua"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "stylua",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "lua"
          ],
          "context": "owned",
          "family": "lua",
          "fix": "supported",
          "state": "qualified",
          "tool": "stylua",
          "transport": "batched"
        }
      ]
    }
  ],
  "make": [
    {
      "adapters": {
        "lint": [
          "make"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "ini",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "checkmake.ini"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".ini"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "checkmake",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "make"
          ],
          "context": "owned",
          "family": "make",
          "fix": "check_only",
          "state": "supported",
          "tool": "checkmake",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "make"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".bake.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".bake.toml",
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "mbake",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "make"
          ],
          "context": "owned",
          "family": "make",
          "fix": "supported",
          "platforms": [
            "linux_x86_64"
          ],
          "state": "supported",
          "tool": "mbake",
          "transport": "batched"
        }
      ]
    }
  ],
  "nix": [
    {
      "adapters": {
        "lint": [
          "nix"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "deadnix",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "nix"
          ],
          "context": "owned",
          "family": "nix",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "deadnix",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "the dx_quality_crates hub has no windows_arm64 cell"
          }
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "nix"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "nixfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "nix"
          ],
          "context": "owned",
          "family": "nix",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "nixfmt",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "nixfmt depends on the unix package and does not build on Windows",
            "windows_x86_64": "nixfmt depends on the unix package and does not build on Windows"
          }
        }
      ]
    }
  ],
  "ocaml": [
    {
      "adapters": {
        "typecheck": [
          "ocaml"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ocaml",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "format": [
          "ocaml"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "topiary",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "ocaml"
          ],
          "context": "owned",
          "family": "ocaml",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "topiary",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "ocaml"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".txt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "zanuda",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "ocaml"
          ],
          "context": "target_coupled",
          "family": "ocaml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "zanuda",
          "transport": "single",
          "unsupported": {
            "windows_arm64": "zanuda has no Windows build",
            "windows_x86_64": "zanuda has no Windows build"
          }
        }
      ]
    }
  ],
  "oci_image": [
    {
      "adapters": {
        "typecheck": [
          "oci_image"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "container_structure_test",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "oci_image"
          ],
          "context": "owned",
          "family": "oci_image",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "container_structure_test",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "oci_image"
        ]
      },
      "concerns": {
        "lint": "security"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "trivy_image_config",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "oci_image"
          ],
          "context": "owned",
          "family": "oci_image",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "trivy_image_config",
          "transport": "single"
        }
      ]
    }
  ],
  "perl": [
    {
      "adapters": {
        "lint": [
          "perl"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".perlcriticrc",
          "perlcriticrc"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "perlcritic",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "perl"
          ],
          "context": "owned",
          "family": "perl",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "perlcritic",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "Windows ARM64 perl is not qualified"
          }
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "perl"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "#",
        "ext": "perltidyrc",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".perltidyrc"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".perltidyrc",
          "perltidyrc"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "perltidy",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "perl"
          ],
          "context": "owned",
          "family": "perl",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "perltidy",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "Windows ARM64 perl is not qualified"
          }
        }
      ]
    }
  ],
  "php": [
    {
      "adapters": {
        "lint": [
          "php"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".xml",
          ".xml.dist"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "phpcs",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "php"
          ],
          "context": "owned",
          "family": "php",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "phpcs",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build"
          }
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "php"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".neon",
          ".neon.dist"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "phpstan level 6 and phpstan-strict-rules"
      },
      "id": "phpstan",
      "severity": {
        "categories": {
          "phpstan/public.": "info"
        },
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "php"
          ],
          "context": "target_coupled",
          "family": "php",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "phpstan",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build"
          }
        }
      ]
    }
  ],
  "pkl": [
    {
      "adapters": {
        "format": [
          "pkl"
        ],
        "typecheck": [
          "pkl"
        ]
      },
      "concerns": {
        "format": "format",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "pkl",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "pkl"
          ],
          "context": "owned",
          "family": "pkl",
          "fix": "supported",
          "state": "qualified",
          "tool": "pkl",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "pkl"
          ],
          "context": "target_coupled",
          "family": "pkl",
          "fix": "check_only",
          "state": "qualified",
          "tool": "pkl",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "pkl"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "pkl_lsp",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "pkl"
          ],
          "context": "owned",
          "family": "pkl",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64",
            "windows_arm64"
          ],
          "state": "supported",
          "tool": "pkl_lsp",
          "transport": "batched"
        }
      ]
    }
  ],
  "powershell": [
    {
      "adapters": {
        "format": [
          "powershell"
        ],
        "lint": [
          "powershell"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "#",
        "ext": "psd1",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "powershell",
        "files": [
          "PSScriptAnalyzerSettings.psd1"
        ],
        "init_order": 6,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".psd1"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "psscriptanalyzer",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "powershell"
          ],
          "context": "owned",
          "family": "powershell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "psscriptanalyzer",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "powershell"
          ],
          "context": "owned",
          "family": "powershell",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "psscriptanalyzer",
          "transport": "batched"
        }
      ]
    }
  ],
  "python": [
    {
      "adapters": {
        "lint": [
          "jupyter"
        ]
      },
      "concerns": {
        "lint": "notebook_outputs"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "notebook_outputs",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "jupyter"
          ],
          "context": "owned",
          "family": "jupyter",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "notebook_outputs",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "python",
          "python_stub"
        ]
      },
      "concerns": {
        "lint": "docstrings"
      },
      "config": {
        "comment": "#",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "pydoclint",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "python",
            "python_stub"
          ],
          "context": "owned",
          "family": "python",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "pydoclint",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "audit": [
          "python",
          "python_stub"
        ],
        "format": [
          "python",
          "python_stub",
          "jupyter"
        ],
        "lint": [
          "python",
          "python_stub",
          "jupyter"
        ]
      },
      "concerns": {
        "audit": "audit",
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "#",
        "ext": "toml",
        "extends_syntax": "# Repo settings go here; the extend line picks the tier.\nextend = \"{base}\"\n",
        "extensions": [],
        "extras": [],
        "family": "python",
        "files": [
          "ruff.toml",
          ".ruff.toml"
        ],
        "init_order": 1,
        "kind": "file",
        "pyproject_table": "ruff",
        "references": [
          "toml:extend"
        ],
        "rule_keys": [
          "lint.select",
          "lint.ignore",
          "lint.per-file-ignores"
        ],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [
          ".py",
          ".pyi"
        ],
        "suppressions": [
          "noqa"
        ],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ruff",
      "severity": {
        "categories": {
          "E9": "error",
          "F82": "error",
          "S1": "error",
          "S2": "error",
          "S3": "error",
          "S4": "error",
          "S5": "error",
          "S6": "error",
          "S7": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "python",
            "python_stub"
          ],
          "context": "owned",
          "family": "python",
          "fix": "supported",
          "state": "qualified",
          "tool": "ruff",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "python",
            "python_stub"
          ],
          "context": "owned",
          "family": "python",
          "fix": "supported",
          "state": "qualified",
          "tool": "ruff",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "jupyter"
          ],
          "context": "owned",
          "family": "jupyter",
          "fix": "supported",
          "state": "qualified",
          "tool": "ruff",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "jupyter"
          ],
          "context": "owned",
          "family": "jupyter",
          "fix": "supported",
          "state": "qualified",
          "tool": "ruff",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "jupyter",
          "python",
          "python_stub"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "ty",
      "severity": {
        "categories": {},
        "default": "error"
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "python",
            "python_stub"
          ],
          "context": "target_coupled",
          "family": "python",
          "fix": "check_only",
          "state": "qualified",
          "tool": "ty",
          "transport": "batched"
        },
        {
          "capability": "typecheck",
          "classes": [
            "jupyter"
          ],
          "context": "target_coupled",
          "family": "jupyter",
          "fix": "check_only",
          "state": "qualified",
          "tool": "ty",
          "transport": "batched"
        }
      ]
    }
  ],
  "rego": [
    {
      "adapters": {
        "format": [
          "rego"
        ],
        "typecheck": [
          "rego"
        ]
      },
      "concerns": {
        "format": "format",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "opa",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "rego"
          ],
          "context": "owned",
          "family": "rego",
          "fix": "supported",
          "state": "qualified",
          "tool": "opa",
          "transport": "single"
        },
        {
          "capability": "typecheck",
          "classes": [
            "rego"
          ],
          "context": "target_coupled",
          "family": "rego",
          "fix": "check_only",
          "state": "qualified",
          "tool": "opa",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "rego"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".regal/config.yaml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": true,
        "suffixes": [
          ".yaml",
          ".yml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "regal",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "rego"
          ],
          "context": "owned",
          "family": "rego",
          "fix": "check_only",
          "state": "qualified",
          "tool": "regal",
          "transport": "batched"
        }
      ]
    }
  ],
  "ruby": [
    {
      "adapters": {
        "format": [
          "ruby"
        ],
        "lint": [
          "ruby"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yml",
          ".yaml"
        ],
        "suppression_extensions": [
          ".rake",
          ".rb"
        ],
        "suppressions": [
          "rubocop:disable"
        ],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "rubocop",
      "severity": {
        "categories": {
          "Security/": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "ruby"
          ],
          "context": "owned",
          "family": "ruby",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "rubocop",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "RubyInstaller arm64 Ruby is not pinned or qualified"
          }
        },
        {
          "capability": "lint",
          "classes": [
            "ruby"
          ],
          "context": "owned",
          "family": "ruby",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "rubocop",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "RubyInstaller arm64 Ruby is not pinned or qualified"
          }
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "ruby"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "sorbet",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "ruby"
          ],
          "context": "target_coupled",
          "family": "ruby",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64"
          ],
          "state": "supported",
          "tool": "sorbet",
          "transport": "batched",
          "unsupported": {
            "windows_arm64": "Sorbet ships no Windows build",
            "windows_x86_64": "Sorbet ships no Windows build"
          }
        }
      ]
    }
  ],
  "rust": [
    {
      "adapters": {
        "lint": [
          "rust"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "flags",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "clippy",
      "severity": {
        "categories": {
          "clippy::": "warning",
          "clippy::absurd_extreme_comparisons": "error",
          "clippy::almost_swapped": "error",
          "clippy::approx_constant": "error",
          "clippy::async_yields_async": "error",
          "clippy::bad_bit_mask": "error",
          "clippy::cast_slice_different_sizes": "error",
          "clippy::char_indices_as_byte_indices": "error",
          "clippy::deprecated_semver": "error",
          "clippy::derive_ord_xor_partial_ord": "error",
          "clippy::derived_hash_with_manual_eq": "error",
          "clippy::eager_transmute": "error",
          "clippy::enum_clike_unportable_variant": "error",
          "clippy::eq_op": "error",
          "clippy::erasing_op": "error",
          "clippy::if_let_mutex": "error",
          "clippy::ifs_same_cond": "error",
          "clippy::impl_hash_borrow_with_str_and_bytes": "error",
          "clippy::impossible_comparisons": "error",
          "clippy::ineffective_bit_mask": "error",
          "clippy::infinite_iter": "error",
          "clippy::inherent_to_string_shadow_display": "error",
          "clippy::inline_fn_without_body": "error",
          "clippy::invalid_regex": "error",
          "clippy::inverted_saturating_sub": "error",
          "clippy::invisible_characters": "error",
          "clippy::iter_next_loop": "error",
          "clippy::iter_skip_zero": "error",
          "clippy::iterator_step_by_zero": "error",
          "clippy::let_underscore_lock": "error",
          "clippy::lint_groups_priority": "error",
          "clippy::match_str_case_mismatch": "error",
          "clippy::mem_replace_with_uninit": "error",
          "clippy::min_max": "error",
          "clippy::mistyped_literal_suffixes": "error",
          "clippy::modulo_one": "error",
          "clippy::mut_from_ref": "error",
          "clippy::never_loop": "error",
          "clippy::non_octal_unix_permissions": "error",
          "clippy::nonsensical_open_options": "error",
          "clippy::not_unsafe_ptr_arg_deref": "error",
          "clippy::option_env_unwrap": "error",
          "clippy::out_of_bounds_indexing": "error",
          "clippy::panicking_overflow_checks": "error",
          "clippy::panicking_unwrap": "error",
          "clippy::possible_missing_comma": "error",
          "clippy::read_line_without_trim": "error",
          "clippy::recursive_format_impl": "error",
          "clippy::redundant_comparisons": "error",
          "clippy::reversed_empty_ranges": "error",
          "clippy::self_assignment": "error",
          "clippy::serde_api_misuse": "error",
          "clippy::size_of_in_element_count": "error",
          "clippy::suspicious_splitn": "error",
          "clippy::transmute_null_to_fn": "error",
          "clippy::transmuting_null": "error",
          "clippy::uninit_assumed_init": "error",
          "clippy::uninit_vec": "error",
          "clippy::unit_cmp": "error",
          "clippy::unit_hash": "error",
          "clippy::unit_return_expecting_ord": "error",
          "clippy::unsound_collection_transmute": "error",
          "clippy::unused_io_amount": "error",
          "clippy::useless_attribute": "error",
          "clippy::vec_resize_to_zero": "error",
          "clippy::while_immutable_condition": "error",
          "clippy::wrong_transmute": "error",
          "clippy::zst_offset": "error"
        },
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "rust"
          ],
          "context": "target_coupled",
          "family": "rust",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "clippy",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "typecheck": [
          "rust"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "rustc",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "typecheck",
          "classes": [
            "rust"
          ],
          "context": "target_coupled",
          "family": "rust",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "rustc",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "rust"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "rustfmt.toml",
          ".rustfmt.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "rustfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "rust"
          ],
          "context": "owned",
          "family": "rust",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "rustfmt",
          "transport": "batched"
        }
      ]
    }
  ],
  "shell": [
    {
      "adapters": {
        "lint": [
          "shell"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".shellcheckrc"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".shellcheckrc",
          "shellcheckrc"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "shellcheck",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "shell"
          ],
          "context": "owned",
          "family": "shell",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "shellcheck",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "shell"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "#",
        "ext": "editorconfig",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".editorconfig"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".editorconfig"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "shfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "shell"
          ],
          "context": "owned",
          "family": "shell",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "shfmt",
          "transport": "single"
        }
      ]
    }
  ],
  "sql": [
    {
      "adapters": {
        "format": [
          "sql"
        ],
        "lint": [
          "sql"
        ]
      },
      "concerns": {
        "format": "format",
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "cfg",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".sqlfluff"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".sqlfluff",
          ".cfg",
          ".ini"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "sqlfluff",
      "severity": {
        "categories": {
          "AM": "error",
          "LXR": "error",
          "PRS": "error",
          "RF": "error",
          "ST": "error",
          "TMP": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "sql"
          ],
          "context": "owned",
          "family": "sql",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "sqlfluff",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "sql"
          ],
          "context": "owned",
          "family": "sql",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "sqlfluff",
          "transport": "batched"
        }
      ]
    }
  ],
  "swift": [
    {
      "adapters": {
        "typecheck": [
          "swift"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "swiftc",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "format": [
          "swift"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".swift-format",
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "swift_format",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "swift"
          ],
          "context": "target_coupled",
          "family": "swift",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "swift_format",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "swift"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".swift-format",
          ".json"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "swift_lint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "swift"
          ],
          "context": "target_coupled",
          "family": "swift",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "swift_lint",
          "transport": "batched"
        }
      ]
    }
  ],
  "terraform": [
    {
      "adapters": {
        "format": [
          "terraform"
        ],
        "typecheck": [
          "terraform"
        ]
      },
      "concerns": {
        "format": "format",
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "terraform",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "terraform"
          ],
          "context": "owned",
          "family": "terraform",
          "fix": "supported",
          "state": "qualified",
          "tool": "terraform",
          "transport": "single"
        },
        {
          "capability": "typecheck",
          "classes": [
            "terraform"
          ],
          "context": "target_coupled",
          "family": "terraform",
          "fix": "check_only",
          "state": "supported",
          "tool": "terraform",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "terraform"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".hcl"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "tflint",
      "severity": {
        "categories": {
          "tflint-diagnostic": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "terraform"
          ],
          "context": "owned",
          "family": "terraform",
          "fix": "check_only",
          "state": "supported",
          "tool": "tflint",
          "transport": "batched"
        }
      ]
    }
  ],
  "text": [
    {
      "adapters": {
        "format": [
          "rst"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "toml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          "docstrfmt.toml"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".toml"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "docstrfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "rst"
          ],
          "context": "owned",
          "family": "rst",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "docstrfmt",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "text"
        ]
      },
      "concerns": {
        "lint": "sorting"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "keep_sorted",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "text"
          ],
          "context": "owned",
          "family": "text",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "qualified",
          "tool": "keep_sorted",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "asciidoc",
          "markdown",
          "rst"
        ]
      },
      "concerns": {
        "lint": "prose"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".vale.ini",
          "_vale.ini"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [
          "ini:StylesPath"
        ],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".ini"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "vale",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "asciidoc"
          ],
          "context": "owned",
          "family": "asciidoc",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "vale",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "markdown"
          ],
          "context": "owned",
          "family": "markdown",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "vale",
          "transport": "batched"
        },
        {
          "capability": "lint",
          "classes": [
            "rst"
          ],
          "context": "owned",
          "family": "rst",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "vale",
          "transport": "batched"
        }
      ]
    }
  ],
  "typst": [
    {
      "adapters": {
        "lint": [
          "typst"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "tinymist",
      "severity": {
        "categories": {},
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "typst"
          ],
          "context": "owned",
          "family": "typst",
          "fix": "check_only",
          "state": "supported",
          "tool": "tinymist",
          "transport": "single"
        }
      ]
    },
    {
      "adapters": {
        "format": [
          "typst"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "typstyle",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "typst"
          ],
          "context": "owned",
          "family": "typst",
          "fix": "supported",
          "state": "supported",
          "tool": "typstyle",
          "transport": "batched"
        }
      ]
    }
  ],
  "xml": [
    {
      "adapters": {
        "lint": [
          "xml"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "xmllint",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "xml"
          ],
          "context": "owned",
          "family": "xml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "xmllint",
          "transport": "batched"
        }
      ]
    }
  ],
  "yaml": [
    {
      "adapters": {
        "format": [
          "ansible",
          "github_workflow",
          "kubernetes",
          "yaml"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "yaml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".yamlfmt"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml",
          ".yml",
          ".yamlfmt"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "yamlfmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "yaml"
          ],
          "context": "owned",
          "family": "yaml",
          "fix": "supported",
          "state": "supported",
          "tool": "yamlfmt",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "github_workflow"
          ],
          "context": "owned",
          "family": "core",
          "fix": "supported",
          "state": "supported",
          "tool": "yamlfmt",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "ansible"
          ],
          "context": "owned",
          "family": "ansible",
          "fix": "supported",
          "state": "supported",
          "tool": "yamlfmt",
          "transport": "batched"
        },
        {
          "capability": "format",
          "classes": [
            "kubernetes"
          ],
          "context": "owned",
          "family": "kubernetes",
          "fix": "supported",
          "state": "supported",
          "tool": "yamlfmt",
          "transport": "batched"
        }
      ]
    },
    {
      "adapters": {
        "lint": [
          "yaml"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "yaml",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [
          ".yamllint"
        ],
        "init_order": 0,
        "kind": "file",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [
          "rules"
        ],
        "shared": false,
        "stage_native": false,
        "suffixes": [
          ".yaml",
          ".yml",
          ".yamllint"
        ],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": true,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "yamllint",
      "severity": {
        "categories": {
          "anchors": "error",
          "key-duplicates": "error",
          "syntax": "error"
        },
        "default": "warning"
      },
      "support": [
        {
          "capability": "lint",
          "classes": [
            "yaml"
          ],
          "context": "owned",
          "family": "yaml",
          "fix": "check_only",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "yamllint",
          "transport": "batched"
        }
      ]
    }
  ],
  "zig": [
    {
      "adapters": {
        "typecheck": [
          "zig"
        ]
      },
      "concerns": {
        "typecheck": "typecheck"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "zig",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "lint": [
          "zig"
        ]
      },
      "concerns": {
        "lint": "lint"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": false,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "zig_ast_check",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": []
    },
    {
      "adapters": {
        "format": [
          "zig"
        ]
      },
      "concerns": {
        "format": "format"
      },
      "config": {
        "comment": "",
        "ext": "",
        "extends_syntax": "",
        "extensions": [],
        "extras": [],
        "family": "",
        "files": [],
        "init_order": 0,
        "kind": "none",
        "pyproject_table": "",
        "references": [],
        "rule_keys": [],
        "shared": false,
        "stage_native": false,
        "suffixes": [],
        "suppression_extensions": [],
        "suppressions": [],
        "tier_active": true,
        "tier_base": false,
        "tier_dir": "",
        "tier_source": "dx"
      },
      "id": "zig_fmt",
      "severity": {
        "categories": {},
        "default": null
      },
      "support": [
        {
          "capability": "format",
          "classes": [
            "zig"
          ],
          "context": "target_coupled",
          "family": "zig",
          "fix": "supported",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "macos_arm64",
            "windows_arm64",
            "windows_x86_64"
          ],
          "state": "supported",
          "tool": "zig_fmt",
          "transport": "single",
          "unsupported": {}
        }
      ]
    }
  ]
}""")
