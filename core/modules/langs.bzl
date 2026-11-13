"""Generated from every <lang>/lang.json by //core/modules:lang_json_test; refresh with UPDATE_EXPECT=1."""

LANGS = json.decode("""{
  "android": {
    "ecosystems": [],
    "entry_points": [
      "android_java_library",
      "android_kotlin_library",
      "android_native_library",
      "android_rust_apk",
      "android_apk_set",
      "android_check",
      "android_sign",
      "android_adb_install",
      "android_adb_start",
      "android_adb_log"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "android",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "setup": {
      "licenses": [
        {
          "id": "android-sdk-license",
          "platforms": [
            "linux_x86_64"
          ]
        }
      ]
    },
    "title": "Android",
    "tools": []
  },
  "ansible": {
    "ecosystems": [
      "ansible"
    ],
    "entry_points": [
      "ansible_library"
    ],
    "family": "core",
    "lockfiles": [
      "ansible.lock.json"
    ],
    "manifests": [
      "requirements.yml"
    ],
    "name": "ansible",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_ansible_lint_aspect"
        ]
      },
      "bzl": "ansible/private/quality:ansible.bzl",
      "every_workspace": true,
      "file_extensions": [
        ".yaml",
        ".yml"
      ]
    },
    "setup": {
      "hubs": [
        {
          "ecosystem": "ansible",
          "extension": "@rules_dx//ansible:extensions.bzl",
          "hub": "ansible_collections",
          "imports": "provides",
          "lock": "ansible.lock.json",
          "manifest": "requirements.yml",
          "resolver": "@rules_dx//ansible/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Ansible",
    "tools": [
      "ansible_lint",
      "yamlfmt"
    ]
  },
  "api_spec": {
    "docs": {
      "aspects": [
        "real_protobuf_docs_aspect"
      ],
      "bzl": "tools/docs/protobuf:aspect.bzl",
      "file_extensions": [
        ".proto"
      ]
    },
    "ecosystems": [],
    "entry_points": [
      "api_spec_go_client",
      "api_spec_library",
      "api_spec_python_client",
      "api_spec_rust_client",
      "api_spec_typescript_client"
    ],
    "family": "generation",
    "lockfiles": [],
    "manifests": [],
    "name": "api_spec",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_protobuf_format_aspect"
        ],
        "lint": [
          "real_protobuf_lint_aspect"
        ],
        "typecheck": [
          "real_protobuf_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/generation.bzl",
      "file_extensions": [
        ".proto"
      ],
      "probes": {
        "format": [
          "real_generation_format_probe_aspect"
        ],
        "lint": [
          "real_generation_lint_probe_aspect"
        ],
        "typecheck": [
          "real_generation_typecheck_probe_aspect"
        ]
      }
    },
    "title": "OpenAPI and AsyncAPI",
    "tools": [
      "prettier",
      "redocly"
    ]
  },
  "apple": {
    "ecosystems": [],
    "entry_points": [
      "apple_ios_app",
      "apple_ios_app_test",
      "apple_ios_xcframework",
      "apple_ios_lib",
      "apple_ios_export",
      "apple_macos_app",
      "apple_macos_sign",
      "apple_macos_notarize",
      "apple_simulator_boot",
      "apple_simulator_install",
      "apple_simulator_launch"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "apple",
    "pins": [],
    "platforms": [
      "macos_arm64"
    ],
    "setup": {
      "licenses": [
        {
          "id": "apple-xcode-license",
          "platforms": [
            "macos_arm64"
          ]
        }
      ]
    },
    "title": "Apple",
    "tools": [],
    "unsupported": {
      "linux_arm64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
      "linux_x86_64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
      "windows_arm64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
      "windows_x86_64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS"
    }
  },
  "asciidoc": {
    "ecosystems": [],
    "entry_points": [
      "asciidoc_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "asciidoc",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "title": "AsciiDoc",
    "tools": [
      "lychee",
      "vale"
    ]
  },
  "astro": {
    "docs": {
      "aspects": [],
      "bzl": "tools/docs/components:aspect.bzl",
      "file_extensions": [
        ".astro"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "astro_library",
      "astro_build",
      "astro_test"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "astro",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".astro"
      ]
    },
    "title": "Astro",
    "tools": [
      "eslint"
    ]
  },
  "avro": {
    "docs": {
      "aspects": [
        "real_avro_docs_aspect"
      ],
      "bzl": "tools/docs/avro:aspect.bzl",
      "file_extensions": [
        ".avdl",
        ".avsc"
      ]
    },
    "ecosystems": [],
    "entry_points": [
      "avro_library",
      "avro_cc_library",
      "avro_java_library"
    ],
    "family": "generation",
    "lockfiles": [],
    "manifests": [],
    "name": "avro",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_avro_lint_aspect"
        ],
        "typecheck": [
          "real_avro_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/generation.bzl",
      "file_extensions": [
        ".avsc"
      ]
    },
    "title": "Avro",
    "tools": [
      "avro_tools",
      "prettier"
    ]
  },
  "bicep": {
    "ecosystems": [
      "bicep"
    ],
    "entry_points": [
      "bicep_library",
      "bicep_registry_module"
    ],
    "family": "core",
    "lockfiles": [
      "bicep.lock.json"
    ],
    "manifests": [
      "*.bicep"
    ],
    "name": "bicep",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_bicep_format_aspect"
        ],
        "lint": [
          "real_bicep_lint_aspect"
        ]
      },
      "bzl": "bicep/private/quality:bicep.bzl",
      "file_extensions": [
        ".bicep",
        ".bicepparam"
      ]
    },
    "setup": {
      "hubs": [
        {
          "ecosystem": "bicep",
          "extension": "@rules_dx//bicep:extensions.bzl",
          "hub": "bicep",
          "imports": "provides",
          "lock": "bicep.lock.json",
          "manifest": "*.bicep",
          "resolver": "@rules_dx//bicep/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Bicep",
    "tools": [
      "bicep",
      "trivy_config"
    ]
  },
  "capnproto": {
    "ecosystems": [],
    "entry_points": [
      "capnproto_cc_library",
      "capnproto_java_library",
      "capnproto_library",
      "capnproto_rust_library"
    ],
    "family": "generation",
    "lockfiles": [],
    "manifests": [],
    "name": "capnproto",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_capnp_lint_aspect"
        ],
        "typecheck": [
          "real_capnp_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/generation.bzl",
      "file_extensions": [
        ".capnp"
      ]
    },
    "title": "Cap'n Proto",
    "tools": []
  },
  "cc": {
    "depcheck": "cc",
    "docs": {
      "aspects": [
        "real_cc_docs_aspect"
      ],
      "bzl": "tools/docs/cc:aspect.bzl",
      "file_extensions": [
        ".c",
        ".cc",
        ".cpp",
        ".cu",
        ".cuh",
        ".cxx",
        ".h",
        ".hh",
        ".hpp",
        ".hxx",
        ".inl",
        ".ipp"
      ]
    },
    "ecosystems": [],
    "entry_points": [
      "cc_library",
      "cc_objc_library",
      "cc_binary",
      "cc_test",
      "cc_fuzz_test",
      "cc_bench"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "cc",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_cc_format_aspect"
        ],
        "lint": [
          "real_cc_lint_aspect"
        ],
        "typecheck": [
          "real_cc_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/cc.bzl",
      "file_extensions": [
        ".c",
        ".cc",
        ".cpp",
        ".cu",
        ".cuh",
        ".cxx",
        ".h",
        ".hh",
        ".hpp",
        ".hxx",
        ".inl",
        ".ipp",
        ".m",
        ".mm"
      ],
      "probes": {
        "format": [
          "real_cc_format_probe_aspect"
        ],
        "lint": [
          "real_cc_lint_probe_aspect"
        ],
        "typecheck": [
          "real_cc_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "licenses": [
        {
          "id": "visual-studio-build-tools",
          "platforms": [
            "windows_arm64",
            "windows_x86_64"
          ]
        }
      ]
    },
    "title": "C and C++",
    "tools": [
      "clang_format",
      "clang_syntax",
      "clang_tidy"
    ]
  },
  "cloudformation": {
    "ecosystems": [],
    "entry_points": [
      "cloudformation_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "cloudformation",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_cloudformation_lint_aspect"
        ]
      },
      "bzl": "cloudformation/private/quality:cloudformation.bzl",
      "every_workspace": true,
      "file_extensions": [
        ".yaml",
        ".yml",
        ".json"
      ]
    },
    "title": "CloudFormation",
    "tools": [
      "cfn_lint",
      "prettier",
      "trivy_config"
    ]
  },
  "cmake": {
    "ecosystems": [],
    "entry_points": [
      "cmake_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "CMakeLists.txt"
    ],
    "name": "cmake",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_cmake_format_aspect"
        ],
        "lint": [
          "real_cmake_lint_aspect"
        ]
      },
      "bzl": "cmake/private/quality:cmake.bzl",
      "file_extensions": [
        ".cmake"
      ],
      "file_names": [
        "CMakeLists.txt"
      ]
    },
    "title": "CMake",
    "tools": [
      "cmakelint",
      "gersemi"
    ]
  },
  "csharp": {
    "depcheck": "csharp",
    "ecosystems": [
      "nuget"
    ],
    "entry_points": [
      "csharp_proto_library",
      "csharp_library",
      "csharp_binary",
      "csharp_test",
      "csharp_bench",
      "csharp_fuzz_test"
    ],
    "family": "core",
    "lockfiles": [
      "paket.lock"
    ],
    "manifests": [
      "paket.dependencies"
    ],
    "name": "csharp",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "nuget",
          "extension": "@rules_dx//csharp:extensions.bzl",
          "hub": "paket.main",
          "imports": "name",
          "lock": "paket.lock",
          "manifest": "paket.dependencies",
          "resolver": null,
          "state": "paket.lock",
          "tag": null
        }
      ]
    },
    "title": "C#",
    "tools": [
      "csc",
      "csharpier",
      "roslyn"
    ]
  },
  "cuda": {
    "ecosystems": [],
    "entry_points": [
      "cuda_library",
      "cuda_binary",
      "cuda_test",
      "cuda_objects",
      "cuda_component_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "cuda",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "setup": {
      "licenses": [
        {
          "id": "cuda-eula",
          "platforms": [
            "linux_arm64",
            "linux_x86_64",
            "windows_arm64",
            "windows_x86_64"
          ]
        }
      ]
    },
    "title": "CUDA",
    "tools": [
      "clang_format",
      "clang_tidy",
      "nvcc_syntax"
    ],
    "unsupported": {
      "macos_arm64": "NVIDIA ships no CUDA toolkit for macOS"
    }
  },
  "d": {
    "depcheck": "d",
    "docs": {
      "aspects": [
        "real_d_docs_aspect"
      ],
      "bzl": "tools/docs/d:aspect.bzl",
      "file_extensions": [
        ".d"
      ]
    },
    "ecosystems": [
      "dub"
    ],
    "entry_points": [
      "d_library",
      "d_binary",
      "d_test",
      "d_fuzz_test",
      "d_bench"
    ],
    "family": "core",
    "lockfiles": [
      "dub.selections.json"
    ],
    "manifests": [
      "dub.json"
    ],
    "name": "d",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_d_format_aspect"
        ],
        "lint": [
          "real_d_lint_aspect"
        ],
        "typecheck": [
          "real_d_typecheck_aspect"
        ]
      },
      "bzl": "d/private/quality:d.bzl",
      "file_extensions": [
        ".d"
      ],
      "probes": {
        "format": [
          "real_d_format_probe_aspect"
        ],
        "lint": [
          "real_d_lint_probe_aspect"
        ],
        "typecheck": [
          "real_d_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "dub",
          "extension": "@rules_dx//d:extensions.bzl",
          "hub": "dub",
          "imports": "provides",
          "lock": "dub.lock.json",
          "manifest": "dub.json",
          "resolver": "@rules_dx//d/private/tools/lock",
          "state": "dub.selections.json",
          "tag": "lock"
        }
      ]
    },
    "title": "D",
    "tools": [
      "ldc"
    ]
  },
  "dart": {
    "depcheck": "dart",
    "docs": {
      "aspects": [
        "real_dart_docs_aspect"
      ],
      "bzl": "tools/docs/dart:aspect.bzl",
      "file_extensions": [
        ".dart"
      ]
    },
    "ecosystems": [
      "pub"
    ],
    "entry_points": [
      "dart_codegen",
      "dart_library",
      "dart_binary",
      "dart_js_binary",
      "dart_wasm_binary",
      "dart_test",
      "dart_bench",
      "dart_config"
    ],
    "family": "core",
    "lockfiles": [
      "pubspec.lock"
    ],
    "manifests": [
      "pubspec.yaml"
    ],
    "name": "dart",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_dart_format_aspect"
        ],
        "lint": [
          "real_dart_lint_aspect"
        ],
        "typecheck": [
          "real_dart_typecheck_aspect"
        ]
      },
      "bzl": "dart/private/quality:dart.bzl",
      "file_extensions": [
        ".dart"
      ],
      "probes": {
        "format": [
          "real_dart_format_probe_aspect"
        ],
        "lint": [
          "real_dart_lint_probe_aspect"
        ],
        "typecheck": [
          "real_dart_typecheck_probe_aspect"
        ]
      }
    },
    "semver": true,
    "setup": {
      "hubs": [
        {
          "ecosystem": "pub",
          "extension": "@rules_dx//dart:extensions.bzl",
          "hub": "pub_deps",
          "imports": "name",
          "lock": "pubspec.lock",
          "manifest": "pubspec.yaml",
          "resolver": null,
          "state": null,
          "tag": "from_lock"
        }
      ]
    },
    "title": "Dart",
    "tools": [
      "dart"
    ],
    "unsupported": {
      "windows_arm64": "rules_dart 0.8.1 ships no Windows arm64 SDK"
    }
  },
  "dockerfile": {
    "ecosystems": [],
    "entry_points": [
      "dockerfile_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "dockerfile",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_dockerfile_lint_aspect"
        ]
      },
      "bzl": "dockerfile/private/quality:dockerfile.bzl",
      "every_workspace": true,
      "file_extensions": []
    },
    "title": "Dockerfile",
    "tools": [
      "hadolint",
      "prettier"
    ]
  },
  "dotnet": {
    "docs": {
      "aspects": [
        "real_dotnet_docs_aspect"
      ],
      "bzl": "tools/docs/dotnet:aspect.bzl",
      "file_extensions": [
        ".cs",
        ".fs"
      ]
    },
    "ecosystems": [
      "nuget"
    ],
    "entry_points": [],
    "family": "core",
    "lockfiles": [
      "paket.lock"
    ],
    "manifests": [
      "paket.dependencies"
    ],
    "name": "dotnet",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_dotnet_format_aspect"
        ],
        "lint": [
          "real_dotnet_lint_aspect"
        ],
        "typecheck": [
          "real_dotnet_typecheck_aspect"
        ]
      },
      "bzl": "dotnet/private/quality:dotnet.bzl",
      "file_extensions": [
        ".cs",
        ".csproj",
        ".fs",
        ".fsproj",
        ".fsx"
      ],
      "probes": {
        "format": [
          "real_dotnet_format_probe_aspect"
        ],
        "lint": [
          "real_dotnet_lint_probe_aspect"
        ],
        "typecheck": [
          "real_dotnet_typecheck_probe_aspect"
        ]
      }
    },
    "title": ".NET",
    "tools": []
  },
  "elixir": {
    "depcheck": "elixir",
    "docs": {
      "aspects": [
        "real_beam_docs_aspect"
      ],
      "bzl": "tools/docs/beam:aspect.bzl",
      "file_extensions": [
        ".ex",
        ".exs"
      ]
    },
    "ecosystems": [
      "hex"
    ],
    "entry_points": [
      "elixir_app",
      "elixir_bench",
      "elixir_escript_archive",
      "elixir_escript_headers",
      "elixir_test"
    ],
    "family": "core",
    "lockfiles": [
      "mix.lock"
    ],
    "manifests": [
      "mix.exs"
    ],
    "name": "elixir",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_elixir_format_aspect"
        ],
        "lint": [
          "real_elixir_lint_aspect"
        ]
      },
      "bzl": "erlang/private/quality:beam.bzl",
      "file_extensions": [
        ".ex",
        ".exs"
      ]
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "hex",
          "extension": "@rules_dx//elixir:extensions.bzl",
          "hub": "hex",
          "imports": "name",
          "lock": "mix.lock",
          "manifest": "mix.exs",
          "resolver": null,
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Elixir",
    "tools": [
      "credo",
      "dialyzer",
      "elixirc",
      "mix_format"
    ]
  },
  "erlang": {
    "depcheck": "erlang",
    "docs": {
      "aspects": [
        "real_beam_docs_aspect"
      ],
      "bzl": "tools/docs/beam:aspect.bzl",
      "file_extensions": [
        ".erl",
        ".hrl"
      ]
    },
    "ecosystems": [
      "hex"
    ],
    "entry_points": [
      "erlang_app",
      "erlang_bench",
      "erlang_bytecode",
      "erlang_ct_suite_test",
      "erlang_erlc_opts",
      "erlang_escript_archive",
      "erlang_eunit_test",
      "erlang_nif",
      "erlang_release",
      "erlang_test_app",
      "erlang_xrl_yrl"
    ],
    "family": "core",
    "lockfiles": [
      "mix.lock"
    ],
    "manifests": [
      "mix.exs"
    ],
    "name": "erlang",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_erlang_format_aspect"
        ],
        "lint": [
          "real_erlang_lint_aspect"
        ],
        "typecheck": [
          "real_erlang_typecheck_aspect"
        ]
      },
      "bzl": "erlang/private/quality:beam.bzl",
      "file_extensions": [
        ".erl",
        ".hrl"
      ],
      "probes": {
        "format": [
          "real_beam_format_probe_aspect"
        ],
        "lint": [
          "real_beam_lint_probe_aspect"
        ],
        "typecheck": [
          "real_beam_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "title": "Erlang",
    "tools": [
      "dialyzer",
      "elvis",
      "erlc",
      "erlfmt"
    ]
  },
  "flatbuffers": {
    "ecosystems": [],
    "entry_points": [
      "flatbuffers_library",
      "flatbuffers_cc_library",
      "flatbuffers_java_library",
      "flatbuffers_rust_library",
      "flatbuffers_conform_test"
    ],
    "family": "generation",
    "lockfiles": [],
    "manifests": [],
    "name": "flatbuffers",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_flatbuffers_lint_aspect"
        ],
        "typecheck": [
          "real_flatbuffers_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/generation.bzl",
      "file_extensions": [
        ".fbs"
      ]
    },
    "title": "FlatBuffers",
    "tools": [
      "flatc"
    ]
  },
  "flutter": {
    "ecosystems": [
      "pub"
    ],
    "entry_points": [
      "flutter_gen_l10n",
      "flutter_library",
      "flutter_test",
      "flutter_application",
      "flutter_web_app",
      "flutter_web_bundle",
      "flutter_android_app",
      "flutter_ios_app"
    ],
    "family": "core",
    "lockfiles": [
      "pubspec.lock"
    ],
    "manifests": [
      "pubspec.yaml"
    ],
    "name": "flutter",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "title": "Flutter",
    "tools": [],
    "unsupported": {
      "windows_arm64": "rules_flutter 0.0.8 and Flutter 3.47.9 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS)"
    }
  },
  "fortran": {
    "depcheck": "fortran",
    "docs": {
      "aspects": [
        "real_fortran_docs_aspect"
      ],
      "bzl": "tools/docs/fortran:aspect.bzl",
      "file_extensions": [
        ".f",
        ".f03",
        ".f08",
        ".f90",
        ".f95"
      ]
    },
    "ecosystems": [
      "fpm"
    ],
    "entry_points": [
      "fortran_library",
      "fortran_binary",
      "fortran_test"
    ],
    "family": "core",
    "lockfiles": [
      "fpm.lock.json"
    ],
    "manifests": [
      "fpm.toml"
    ],
    "name": "fortran",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_fortran_format_aspect"
        ],
        "lint": [
          "real_fortran_lint_aspect"
        ],
        "typecheck": [
          "real_fortran_typecheck_aspect"
        ]
      },
      "bzl": "fortran/private/quality:fortran.bzl",
      "file_extensions": [
        ".f",
        ".f03",
        ".f08",
        ".f90",
        ".f95"
      ],
      "probes": {
        "format": [
          "real_fortran_format_probe_aspect"
        ],
        "lint": [
          "real_fortran_lint_probe_aspect"
        ],
        "typecheck": [
          "real_fortran_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "fpm",
          "extension": "@rules_dx//fortran:extensions.bzl",
          "hub": "fpm",
          "imports": "provides",
          "lock": "fpm.lock.json",
          "manifest": "fpm.toml",
          "resolver": "@rules_dx//fortran/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Fortran",
    "tools": [
      "ffmt",
      "fortitude"
    ],
    "unsupported": {
      "windows_arm64": "flang-releases v21.1.8 ships no Windows arm64 build"
    }
  },
  "fsharp": {
    "depcheck": "fsharp",
    "ecosystems": [
      "nuget"
    ],
    "entry_points": [
      "fsharp_library",
      "fsharp_binary",
      "fsharp_test",
      "fsharp_bench",
      "fsharp_fuzz_test"
    ],
    "family": "core",
    "lockfiles": [
      "paket.lock"
    ],
    "manifests": [
      "paket.dependencies"
    ],
    "name": "fsharp",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "semver": false,
    "title": "F#",
    "tools": [
      "fantomas",
      "fsc",
      "fsharplint"
    ]
  },
  "go": {
    "depcheck": "go",
    "docs": {
      "aspects": [
        "real_go_docs_aspect"
      ],
      "bzl": "tools/docs/go:aspect.bzl",
      "file_extensions": [
        ".go"
      ]
    },
    "ecosystems": [
      "go"
    ],
    "entry_points": [
      "go_library",
      "go_binary",
      "go_test",
      "go_fuzz_test",
      "go_bench"
    ],
    "family": "core",
    "lockfiles": [
      "go.sum"
    ],
    "manifests": [
      "go.mod"
    ],
    "name": "go",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_go_format_aspect"
        ],
        "lint": [
          "real_go_lint_aspect"
        ],
        "typecheck": [
          "real_go_typecheck_aspect"
        ]
      },
      "bzl": "go/private/quality:go.bzl",
      "file_extensions": [
        ".go"
      ],
      "probes": {
        "format": [
          "real_go_format_probe_aspect"
        ],
        "lint": [
          "real_go_lint_probe_aspect"
        ],
        "typecheck": [
          "real_go_typecheck_probe_aspect"
        ]
      }
    },
    "semver": true,
    "setup": {
      "hubs": [
        {
          "ecosystem": "go",
          "extension": "@rules_dx//go:extensions.bzl",
          "hub": "*",
          "imports": "name",
          "lock": "go.sum",
          "manifest": "go.mod",
          "resolver": null,
          "state": null,
          "tag": "from_file"
        }
      ]
    },
    "title": "Go",
    "tools": [
      "errcheck",
      "gofumpt",
      "gotype",
      "govet",
      "staticcheck"
    ]
  },
  "haskell": {
    "depcheck": "haskell",
    "docs": {
      "aspects": [
        "real_haskell_docs_aspect"
      ],
      "bzl": "tools/docs/haskell:aspect.bzl",
      "file_extensions": [
        ".hs"
      ]
    },
    "ecosystems": [
      "cabal"
    ],
    "entry_points": [
      "haskell_library",
      "haskell_binary",
      "haskell_test",
      "haskell_bench",
      "haskell_deps_resolve",
      "haskell_deps_resolve_test"
    ],
    "family": "core",
    "lockfiles": [
      "cabal.project.freeze"
    ],
    "manifests": [
      "cabal.project"
    ],
    "name": "haskell",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_haskell_format_aspect"
        ],
        "lint": [
          "real_haskell_lint_aspect"
        ],
        "typecheck": [
          "real_haskell_typecheck_aspect"
        ]
      },
      "bzl": "haskell/private/quality:haskell.bzl",
      "file_extensions": [
        ".hs"
      ],
      "probes": {
        "format": [
          "real_haskell_format_probe_aspect"
        ],
        "lint": [
          "real_haskell_lint_probe_aspect"
        ],
        "typecheck": [
          "real_haskell_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "cabal",
          "extension": "@rules_dx//haskell:extensions.bzl",
          "hub": "hackage",
          "imports": "name",
          "lock": "haskell_deps.json",
          "manifest": "cabal.project",
          "resolver": null,
          "state": "cabal.project.freeze",
          "tag": "lock"
        }
      ]
    },
    "title": "Haskell",
    "tools": [
      "hlint",
      "ormolu",
      "weeder"
    ],
    "unsupported": {
      "windows_arm64": "GHC 9.10.3 ships no Windows arm64 bindist"
    }
  },
  "helm": {
    "ecosystems": [],
    "entry_points": [
      "helm_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "Chart.yaml"
    ],
    "name": "helm",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "title": "Helm",
    "tools": [
      "helm",
      "prettier",
      "trivy_config"
    ]
  },
  "java": {
    "depcheck": "java",
    "docs": {
      "aspects": [
        "real_jvm_docs_aspect"
      ],
      "bzl": "tools/docs/jvm:aspect.bzl",
      "file_extensions": [
        ".java"
      ]
    },
    "ecosystems": [
      "maven"
    ],
    "entry_points": [
      "java_library",
      "java_binary",
      "java_test",
      "java_fuzz_test",
      "java_bench"
    ],
    "family": "core",
    "lockfiles": [
      "maven_install.json"
    ],
    "manifests": [
      "libs.versions.toml"
    ],
    "name": "java",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_jvm_format_aspect"
        ],
        "lint": [
          "real_jvm_lint_aspect",
          "real_pkl_lint_aspect"
        ],
        "typecheck": [
          "real_jvm_typecheck_aspect"
        ]
      },
      "bzl": "java/private/quality:jvm.bzl",
      "file_extensions": [
        ".java"
      ],
      "probes": {
        "format": [
          "real_jvm_format_probe_aspect"
        ],
        "lint": [
          "real_jvm_lint_probe_aspect"
        ],
        "typecheck": [
          "real_jvm_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "maven",
          "extension": "@rules_dx//java:extensions.bzl",
          "hub": "maven",
          "imports": "provides",
          "lock": "maven_install.json",
          "manifest": "libs.versions.toml",
          "resolver": null,
          "state": null,
          "tag": "from_toml"
        }
      ]
    },
    "title": "Java",
    "tools": [
      "google_java_format",
      "javac",
      "pmd",
      "spotbugs"
    ]
  },
  "javascript": {
    "depcheck": "js",
    "docs": {
      "aspects": [
        "real_javascript_docs_aspect"
      ],
      "bzl": "tools/docs/javascript:aspect.bzl",
      "file_extensions": [
        ".cjs",
        ".js",
        ".jsx",
        ".mjs"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "javascript_library",
      "javascript_wasm_bindgen_library",
      "javascript_binary",
      "javascript_test",
      "javascript_bench",
      "javascript_fuzz_test",
      "javascript_web_app",
      "javascript_web_bundle"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "javascript",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_js_format_aspect"
        ],
        "lint": [
          "real_js_lint_aspect"
        ],
        "typecheck": [
          "real_js_typecheck_aspect"
        ]
      },
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".cjs",
        ".js",
        ".jsx",
        ".mjs"
      ],
      "probes": {
        "format": [
          "real_js_format_probe_aspect"
        ],
        "lint": [
          "real_js_lint_probe_aspect"
        ],
        "typecheck": [
          "real_js_typecheck_probe_aspect"
        ]
      }
    },
    "semver": true,
    "setup": {
      "hubs": [
        {
          "ecosystem": "npm",
          "extension": "@rules_dx//javascript:extensions.bzl",
          "hub": "npm",
          "imports": "name",
          "lock": "pnpm-lock.yaml",
          "manifest": "package.json",
          "resolver": null,
          "state": null,
          "tag": "npm_translate_lock"
        },
        {
          "ecosystem": "pnpm",
          "extension": "@rules_dx//javascript:extensions.bzl",
          "hub": "pnpm",
          "imports": "name",
          "lock": null,
          "manifest": "package.json",
          "resolver": null,
          "state": null,
          "tag": null
        }
      ]
    },
    "title": "JavaScript",
    "tools": [
      "biome",
      "eslint"
    ]
  },
  "julia": {
    "depcheck": "julia",
    "docs": {
      "aspects": [
        "real_julia_docs_aspect"
      ],
      "bzl": "tools/docs/julia:aspect.bzl",
      "file_extensions": [
        ".jl"
      ]
    },
    "ecosystems": [
      "julia"
    ],
    "entry_points": [
      "julia_library",
      "julia_binary",
      "julia_test",
      "julia_standalone_binary",
      "julia_bench"
    ],
    "family": "core",
    "lockfiles": [
      "Manifest.bazel.json"
    ],
    "manifests": [
      "Project.toml"
    ],
    "name": "julia",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_julia_format_aspect"
        ],
        "lint": [
          "real_julia_lint_aspect"
        ],
        "typecheck": [
          "real_julia_typecheck_aspect"
        ]
      },
      "bzl": "julia/private/quality:julia.bzl",
      "file_extensions": [
        ".jl"
      ],
      "probes": {
        "format": [
          "real_julia_format_probe_aspect"
        ],
        "lint": [
          "real_julia_lint_probe_aspect"
        ],
        "typecheck": [
          "real_julia_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "julia",
          "extension": "@rules_dx//julia:extensions.bzl",
          "hub": "julia",
          "imports": "name",
          "lock": "Manifest.bazel.json",
          "manifest": "Project.toml",
          "resolver": null,
          "state": "Manifest.toml",
          "tag": "from_manifest"
        }
      ]
    },
    "title": "Julia",
    "tools": [
      "julia_format",
      "julialint"
    ],
    "unsupported": {
      "windows_arm64": "julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build"
    }
  },
  "kotlin": {
    "depcheck": "kotlin",
    "docs": {
      "aspects": [],
      "bzl": "tools/docs/jvm:aspect.bzl",
      "file_extensions": [
        ".kt"
      ]
    },
    "ecosystems": [
      "maven"
    ],
    "entry_points": [
      "kotlin_library",
      "kotlin_binary",
      "kotlin_test",
      "kotlin_bench",
      "kotlin_fuzz_test"
    ],
    "family": "core",
    "lockfiles": [
      "maven_install.json"
    ],
    "manifests": [
      "libs.versions.toml"
    ],
    "name": "kotlin",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "java/private/quality:jvm.bzl",
      "file_extensions": [
        ".kt",
        ".kts"
      ]
    },
    "semver": false,
    "title": "Kotlin",
    "tools": [
      "detekt",
      "kotlinc",
      "ktfmt"
    ]
  },
  "kubernetes": {
    "ecosystems": [],
    "entry_points": [
      "kubernetes_library",
      "kubernetes_schemas"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "kubernetes",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_kubernetes_format_aspect"
        ],
        "lint": [
          "real_kubernetes_lint_aspect"
        ]
      },
      "bzl": "kubernetes/private/quality:kubernetes.bzl",
      "every_workspace": true,
      "file_extensions": [
        ".yaml",
        ".yml"
      ]
    },
    "title": "Kubernetes",
    "tools": [
      "kubeconform",
      "trivy_config",
      "yamlfmt"
    ]
  },
  "lua": {
    "depcheck": "lua",
    "docs": {
      "aspects": [
        "real_lua_docs_aspect"
      ],
      "bzl": "tools/docs/lua:aspect.bzl",
      "file_extensions": [
        ".lua"
      ]
    },
    "ecosystems": [
      "luarocks"
    ],
    "entry_points": [
      "lua_library",
      "lua_binary",
      "lua_test",
      "lua_bench"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "*.rockspec"
    ],
    "name": "lua",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_lua_format_aspect"
        ],
        "lint": [
          "real_lua_lint_aspect"
        ],
        "typecheck": [
          "real_lua_typecheck_aspect"
        ]
      },
      "bzl": "lua/private/quality:lua.bzl",
      "file_extensions": [
        ".lua"
      ],
      "probes": {
        "format": [
          "real_lua_format_probe_aspect"
        ],
        "lint": [
          "real_lua_lint_probe_aspect"
        ],
        "typecheck": [
          "real_lua_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "luarocks",
          "extension": "@rules_dx//lua:extensions.bzl",
          "hub": "rocks",
          "imports": "provides",
          "lock": "luarocks.lock.json",
          "manifest": "*.rockspec",
          "resolver": "@rules_dx//lua/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Lua",
    "tools": [
      "selene",
      "stylua"
    ]
  },
  "make": {
    "ecosystems": [],
    "entry_points": [
      "make_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "make",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_make_format_aspect"
        ],
        "lint": [
          "real_make_lint_aspect"
        ]
      },
      "bzl": "make/private/quality:make.bzl",
      "file_extensions": [
        ".mk"
      ],
      "file_names": [
        "GNUmakefile",
        "Makefile",
        "makefile"
      ]
    },
    "title": "Make",
    "tools": [
      "checkmake",
      "mbake"
    ]
  },
  "mdx": {
    "docs": {
      "aspects": [],
      "bzl": "tools/docs/components:aspect.bzl",
      "file_extensions": [
        ".mdx"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "mdx_library",
      "mdx_build",
      "mdx_test"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "mdx",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".mdx"
      ]
    },
    "title": "MDX",
    "tools": [
      "eslint",
      "lychee",
      "remark"
    ]
  },
  "msix": {
    "ecosystems": [],
    "entry_points": [
      "msix_package",
      "msix_sign"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "msix",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "windows_arm64",
      "windows_x86_64"
    ],
    "setup": {
      "licenses": [
        {
          "id": "windows-sdk-buildtools",
          "platforms": [
            "windows_arm64",
            "windows_x86_64"
          ]
        }
      ]
    },
    "title": "MSIX",
    "tools": [],
    "unsupported": {
      "linux_arm64": "MakeAppx and SignTool from the Windows SDK run only on Windows",
      "linux_x86_64": "MakeAppx and SignTool from the Windows SDK run only on Windows",
      "macos_arm64": "MakeAppx and SignTool from the Windows SDK run only on Windows"
    }
  },
  "ocaml": {
    "depcheck": "ocaml",
    "docs": {
      "aspects": [
        "real_ocaml_docs_aspect"
      ],
      "bzl": "tools/docs/ocaml:aspect.bzl",
      "file_extensions": [
        ".ml",
        ".mli"
      ]
    },
    "ecosystems": [
      "opam"
    ],
    "entry_points": [
      "ocaml_lex",
      "ocaml_module",
      "ocaml_signature",
      "ocaml_library",
      "ocaml_binary",
      "ocaml_test",
      "ocaml_fuzz_test",
      "ocaml_bench",
      "ocaml_dune_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "*.opam"
    ],
    "name": "ocaml",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_ocaml_format_aspect"
        ],
        "lint": [
          "real_ocaml_lint_aspect"
        ],
        "typecheck": [
          "real_ocaml_typecheck_aspect"
        ]
      },
      "bzl": "ocaml/private/quality:ocaml.bzl",
      "file_extensions": [
        ".ml",
        ".mli"
      ],
      "probes": {
        "format": [
          "real_ocaml_format_probe_aspect"
        ],
        "lint": [
          "real_ocaml_lint_probe_aspect"
        ],
        "typecheck": [
          "real_ocaml_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "opam",
          "extension": "@rules_dx//ocaml:extensions.bzl",
          "hub": "opam",
          "imports": "name",
          "lock": "*.opam.locked",
          "manifest": "*.opam",
          "resolver": null,
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "OCaml",
    "tools": [
      "topiary",
      "zanuda"
    ],
    "unsupported": {
      "windows_arm64": "conda-forge ships no win-arm64 OCaml package",
      "windows_x86_64": "conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2"
    }
  },
  "perl": {
    "depcheck": "perl",
    "docs": {
      "aspects": [
        "real_perl_docs_aspect"
      ],
      "bzl": "tools/docs/perl:aspect.bzl",
      "file_extensions": [
        ".pl",
        ".pm"
      ]
    },
    "ecosystems": [
      "cpan"
    ],
    "entry_points": [
      "perl_library",
      "perl_binary",
      "perl_test",
      "perl_bench"
    ],
    "family": "core",
    "lockfiles": [
      "cpanfile.snapshot"
    ],
    "manifests": [
      "cpanfile"
    ],
    "name": "perl",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_perl_format_aspect"
        ],
        "lint": [
          "real_perl_lint_aspect"
        ]
      },
      "bzl": "perl/private/quality:perl.bzl",
      "file_extensions": [
        ".pl",
        ".pm"
      ],
      "probes": {
        "format": [
          "real_perl_format_probe_aspect"
        ],
        "lint": [
          "real_perl_lint_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "cpan",
          "extension": "@rules_dx//perl:extensions.bzl",
          "hub": "cpan",
          "imports": "provides",
          "lock": "cpan.lock.json",
          "manifest": "cpanfile",
          "resolver": "@rules_dx//perl/private/tools/lock",
          "state": "cpanfile.snapshot",
          "tag": "lock"
        }
      ]
    },
    "title": "Perl",
    "tools": [
      "perlcritic",
      "perltidy"
    ],
    "unsupported": {
      "windows_arm64": "Windows ARM64 perl is not qualified"
    }
  },
  "php": {
    "depcheck": "php",
    "docs": {
      "aspects": [
        "real_php_docs_aspect"
      ],
      "bzl": "tools/docs/php:aspect.bzl",
      "file_extensions": [
        ".php"
      ]
    },
    "ecosystems": [
      "composer"
    ],
    "entry_points": [
      "php_library",
      "php_binary",
      "php_test",
      "php_bench"
    ],
    "family": "core",
    "lockfiles": [
      "composer.lock"
    ],
    "manifests": [
      "composer.json"
    ],
    "name": "php",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_php_lint_aspect"
        ],
        "typecheck": [
          "real_php_typecheck_aspect"
        ]
      },
      "bzl": "php/private/quality:php.bzl",
      "file_extensions": [
        ".php"
      ],
      "probes": {
        "lint": [
          "real_php_lint_probe_aspect"
        ],
        "typecheck": [
          "real_php_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "composer",
          "extension": "@rules_dx//php:extensions.bzl",
          "hub": "composer",
          "imports": "provides",
          "lock": "composer.lock.json",
          "manifest": "composer.json",
          "resolver": "@rules_dx//php/private/tools/lock",
          "state": "composer.lock",
          "tag": "lock"
        }
      ]
    },
    "title": "PHP",
    "tools": [
      "phpcs",
      "phpstan",
      "prettier"
    ],
    "unsupported": {
      "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build"
    }
  },
  "powershell": {
    "depcheck": "powershell",
    "docs": {
      "aspects": [
        "real_powershell_docs_aspect"
      ],
      "bzl": "tools/docs/powershell:aspect.bzl",
      "file_extensions": [
        ".ps1",
        ".psd1",
        ".psm1"
      ]
    },
    "ecosystems": [
      "powershell"
    ],
    "entry_points": [
      "powershell_library",
      "powershell_binary",
      "powershell_test"
    ],
    "family": "core",
    "lockfiles": [
      "powershell.lock.json"
    ],
    "manifests": [
      "PSGallery.requirements.psd1"
    ],
    "name": "powershell",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_powershell_format_aspect"
        ],
        "lint": [
          "real_powershell_lint_aspect"
        ]
      },
      "bzl": "powershell/private/quality:powershell.bzl",
      "file_extensions": [
        ".ps1",
        ".psd1",
        ".psm1"
      ],
      "probes": {
        "format": [
          "real_powershell_format_probe_aspect"
        ],
        "lint": [
          "real_powershell_lint_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "powershell",
          "extension": "@rules_dx//powershell:extensions.bzl",
          "hub": "psgallery",
          "imports": "provides",
          "lock": "powershell.lock.json",
          "manifest": "PSGallery.requirements.psd1",
          "resolver": "@rules_dx//powershell/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "PowerShell",
    "tools": [
      "psscriptanalyzer"
    ]
  },
  "python": {
    "depcheck": "python",
    "docs": {
      "aspects": [
        "real_python_docs_aspect"
      ],
      "bzl": "tools/docs/python:aspect.bzl",
      "file_extensions": [
        ".py",
        ".pyi"
      ]
    },
    "ecosystems": [
      "uv"
    ],
    "entry_points": [
      "python_library",
      "python_binary",
      "python_test",
      "python_fuzz_test",
      "python_bench",
      "python_grpc_library"
    ],
    "family": "python",
    "lockfiles": [
      "uv.lock"
    ],
    "manifests": [
      "pyproject.toml"
    ],
    "name": "python",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_python_format_aspect"
        ],
        "lint": [
          "real_python_lint_aspect"
        ],
        "typecheck": [
          "real_python_typecheck_aspect"
        ]
      },
      "bzl": "python/private/quality:python.bzl",
      "file_extensions": [
        ".py",
        ".pyi"
      ],
      "probes": {
        "format": [
          "real_python_format_probe_aspect"
        ],
        "lint": [
          "real_python_lint_probe_aspect"
        ],
        "typecheck": [
          "real_python_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "uv",
          "extension": "@rules_dx//python:extensions.bzl",
          "hub": "pypi",
          "imports": "name",
          "lock": "uv.lock",
          "manifest": "pyproject.toml",
          "resolver": null,
          "state": null,
          "tag": "project"
        }
      ]
    },
    "title": "Python",
    "tools": [
      "pydoclint",
      "ruff",
      "ty"
    ]
  },
  "qt": {
    "ecosystems": [],
    "entry_points": [
      "qt_frameworks",
      "qt_cc_library",
      "qt_qml_module",
      "qt_cc_binary",
      "qt_cc_test",
      "qt_rust_cc_library",
      "qt_bundle",
      "qt_consumer_archive",
      "qt_rust_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "qt",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "core/quality:families/cc.bzl",
      "file_extensions": [
        ".qml"
      ]
    },
    "title": "Qt",
    "tools": [
      "qmlformat",
      "qmllint"
    ]
  },
  "rego": {
    "ecosystems": [],
    "entry_points": [
      "rego_library",
      "rego_test"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "rego",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_rego_format_aspect"
        ],
        "lint": [
          "real_rego_lint_aspect"
        ],
        "typecheck": [
          "real_rego_typecheck_aspect"
        ]
      },
      "bzl": "rego/private/quality:rego.bzl",
      "file_extensions": [
        ".rego"
      ]
    },
    "title": "Rego",
    "tools": [
      "opa",
      "regal"
    ]
  },
  "rst": {
    "ecosystems": [],
    "entry_points": [
      "rst_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "rst",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "title": "reStructuredText",
    "tools": [
      "docstrfmt",
      "lychee",
      "vale"
    ]
  },
  "ruby": {
    "depcheck": "ruby",
    "docs": {
      "aspects": [
        "real_ruby_docs_aspect"
      ],
      "bzl": "tools/docs/ruby:aspect.bzl",
      "file_extensions": [
        ".rb"
      ]
    },
    "ecosystems": [
      "ruby"
    ],
    "entry_points": [
      "ruby_library",
      "ruby_binary",
      "ruby_test",
      "ruby_bench"
    ],
    "family": "core",
    "lockfiles": [
      "Gemfile.lock"
    ],
    "manifests": [
      "Gemfile"
    ],
    "name": "ruby",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_ruby_format_aspect"
        ],
        "lint": [
          "real_ruby_lint_aspect"
        ],
        "typecheck": [
          "real_ruby_typecheck_aspect"
        ]
      },
      "bzl": "ruby/private/quality:ruby.bzl",
      "file_extensions": [
        ".rb",
        ".rbs"
      ],
      "probes": {
        "format": [
          "real_ruby_format_probe_aspect"
        ],
        "lint": [
          "real_ruby_lint_probe_aspect"
        ],
        "typecheck": [
          "real_ruby_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "ruby",
          "extension": "@rules_dx//ruby:extensions.bzl",
          "hub": "bundle",
          "imports": "name",
          "lock": "Gemfile.lock",
          "manifest": "Gemfile",
          "resolver": null,
          "state": null,
          "tag": "bundle_fetch"
        }
      ]
    },
    "title": "Ruby",
    "tools": [
      "rubocop",
      "sorbet"
    ]
  },
  "rust": {
    "depcheck": "rust",
    "docs": {
      "aspects": [
        "real_rust_docs_aspect"
      ],
      "bzl": "tools/docs/rust:aspect.bzl",
      "file_extensions": [
        ".rs"
      ]
    },
    "ecosystems": [
      "cargo"
    ],
    "entry_points": [
      "rust_library",
      "rust_binary",
      "rust_test",
      "rust_bench",
      "rust_proc_macro",
      "rust_shared_library",
      "rust_static_library",
      "rust_fuzz_test",
      "rust_build_script"
    ],
    "family": "core",
    "lockfiles": [
      "Cargo.lock"
    ],
    "manifests": [
      "Cargo.toml"
    ],
    "name": "rust",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_rust_format_aspect"
        ],
        "lint": [
          "real_rust_lint_aspect"
        ],
        "typecheck": [
          "real_rust_typecheck_aspect"
        ]
      },
      "bzl": "rust/private/quality:rust.bzl",
      "file_extensions": [
        ".rs"
      ],
      "probes": {
        "format": [
          "real_rust_format_probe_aspect"
        ],
        "lint": [
          "real_rust_lint_probe_aspect"
        ],
        "typecheck": [
          "real_rust_typecheck_probe_aspect"
        ]
      }
    },
    "semver": true,
    "setup": {
      "hubs": [
        {
          "ecosystem": "cargo",
          "extension": "@rules_dx//rust:extensions.bzl",
          "hub": "crates",
          "imports": "name",
          "lock": "Cargo.lock",
          "manifest": "Cargo.toml",
          "resolver": null,
          "state": null,
          "tag": "from_cargo"
        }
      ]
    },
    "title": "Rust",
    "tools": [
      "clippy",
      "rustc",
      "rustfmt"
    ]
  },
  "scala": {
    "depcheck": "scala",
    "docs": {
      "aspects": [],
      "bzl": "tools/docs/jvm:aspect.bzl",
      "file_extensions": [
        ".scala"
      ]
    },
    "ecosystems": [
      "maven"
    ],
    "entry_points": [
      "scala_library",
      "scala_binary",
      "scala_test",
      "scala_fuzz_test",
      "scala_bench"
    ],
    "family": "core",
    "lockfiles": [
      "maven_install.json"
    ],
    "manifests": [
      "libs.versions.toml"
    ],
    "name": "scala",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_scala_format_aspect"
        ],
        "lint": [
          "real_scala_lint_aspect"
        ]
      },
      "bzl": "java/private/quality:jvm.bzl",
      "file_extensions": [
        ".sc",
        ".scala"
      ]
    },
    "semver": false,
    "title": "Scala",
    "tools": [
      "scalac",
      "scalafix",
      "scalafmt"
    ]
  },
  "sql": {
    "ecosystems": [],
    "entry_points": [
      "sql_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "sql",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_sql_format_aspect"
        ],
        "lint": [
          "real_sql_lint_aspect"
        ]
      },
      "bzl": "sql/private/quality:sql.bzl",
      "file_extensions": [
        ".sql"
      ]
    },
    "title": "SQL",
    "tools": [
      "sqlfluff"
    ]
  },
  "svelte": {
    "docs": {
      "aspects": [],
      "bzl": "tools/docs/components:aspect.bzl",
      "file_extensions": [
        ".svelte"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "svelte_library",
      "svelte_build",
      "svelte_test"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "svelte",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".svelte"
      ]
    },
    "title": "Svelte",
    "tools": [
      "eslint"
    ]
  },
  "swift": {
    "depcheck": "swift",
    "docs": {
      "aspects": [
        "real_swift_docs_aspect"
      ],
      "bzl": "tools/docs/swift:aspect.bzl",
      "file_extensions": [
        ".swift"
      ]
    },
    "ecosystems": [
      "swiftpm"
    ],
    "entry_points": [
      "swift_interop_hint",
      "swift_library",
      "swift_library_group",
      "swift_binary",
      "swift_test",
      "swift_fuzz_test",
      "swift_bench"
    ],
    "family": "core",
    "lockfiles": [
      "Package.resolved"
    ],
    "manifests": [
      "Package.swift"
    ],
    "name": "swift",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_swift_format_aspect"
        ],
        "lint": [
          "real_swift_lint_aspect"
        ],
        "typecheck": [
          "real_swift_typecheck_aspect"
        ]
      },
      "bzl": "swift/private/quality:swift.bzl",
      "file_extensions": [
        ".swift"
      ],
      "probes": {
        "format": [
          "real_swift_format_probe_aspect"
        ],
        "lint": [
          "real_swift_lint_probe_aspect"
        ],
        "typecheck": [
          "real_swift_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "swiftpm",
          "extension": "@rules_dx//swift:extensions.bzl",
          "hub": "swiftpkg",
          "imports": "provides",
          "lock": "swiftpm.lock.json",
          "manifest": "Package.swift",
          "resolver": "@rules_dx//swift/private/tools/lock",
          "state": "Package.resolved",
          "tag": "lock"
        }
      ]
    },
    "title": "Swift",
    "tools": [
      "swift_format",
      "swift_lint"
    ],
    "unsupported": {
      "windows_arm64": "The Swift Windows arm64 toolchain is registered but unverified on Windows ARM",
      "windows_x86_64": "rules_dx registers no Swift toolchain for Windows"
    }
  },
  "terraform": {
    "ecosystems": [],
    "entry_points": [
      "terraform_library",
      "terraform_provider"
    ],
    "family": "core",
    "lockfiles": [
      ".terraform.lock.hcl"
    ],
    "manifests": [
      "*.tf"
    ],
    "name": "terraform",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_terraform_format_aspect"
        ],
        "lint": [
          "real_terraform_lint_aspect"
        ],
        "typecheck": [
          "real_terraform_typecheck_aspect"
        ]
      },
      "bzl": "terraform/private/quality:terraform.bzl",
      "file_extensions": [
        ".tf",
        ".tfvars"
      ]
    },
    "title": "Terraform",
    "tools": [
      "terraform",
      "tflint",
      "trivy_config"
    ]
  },
  "thrift": {
    "ecosystems": [],
    "entry_points": [
      "thrift_library",
      "thrift_cc_library",
      "thrift_java_library",
      "thrift_rust_library"
    ],
    "family": "generation",
    "lockfiles": [],
    "manifests": [],
    "name": "thrift",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "lint": [
          "real_thrift_lint_aspect"
        ],
        "typecheck": [
          "real_thrift_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/generation.bzl",
      "file_extensions": [
        ".thrift"
      ]
    },
    "title": "Thrift",
    "tools": [
      "thrift"
    ]
  },
  "typescript": {
    "depcheck": "ts",
    "docs": {
      "aspects": [
        "real_typescript_docs_aspect"
      ],
      "bzl": "tools/docs/typescript:aspect.bzl",
      "file_extensions": [
        ".cts",
        ".mts",
        ".ts",
        ".tsx"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "typescript_library",
      "typescript_proto_library",
      "typescript_binary",
      "typescript_test",
      "typescript_bench",
      "typescript_fuzz_test",
      "typescript_playwright_test"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "typescript",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".cts",
        ".mts",
        ".ts",
        ".tsx"
      ]
    },
    "semver": true,
    "title": "TypeScript",
    "tools": [
      "biome",
      "eslint"
    ]
  },
  "vue": {
    "docs": {
      "aspects": [
        "real_components_docs_aspect"
      ],
      "bzl": "tools/docs/components:aspect.bzl",
      "file_extensions": [
        ".vue"
      ]
    },
    "ecosystems": [
      "npm"
    ],
    "entry_points": [
      "vue_library",
      "vue_build",
      "vue_test"
    ],
    "family": "core",
    "lockfiles": [
      "pnpm-lock.yaml"
    ],
    "manifests": [
      "package.json"
    ],
    "name": "vue",
    "pins": [],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "bzl": "javascript/private/quality:js.bzl",
      "file_extensions": [
        ".vue"
      ]
    },
    "title": "Vue",
    "tools": [
      "eslint"
    ]
  },
  "zig": {
    "depcheck": "zig",
    "docs": {
      "aspects": [
        "real_zig_docs_aspect"
      ],
      "bzl": "tools/docs/zig:aspect.bzl",
      "file_extensions": [
        ".zig"
      ]
    },
    "ecosystems": [
      "zig"
    ],
    "entry_points": [
      "zig_library",
      "zig_binary",
      "zig_test",
      "zig_bench"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "build.zig.zon"
    ],
    "name": "zig",
    "pins": [
      "pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64",
      "windows_arm64",
      "windows_x86_64"
    ],
    "quality": {
      "aspects": {
        "format": [
          "real_zig_format_aspect"
        ],
        "lint": [
          "real_zig_lint_aspect"
        ],
        "typecheck": [
          "real_zig_typecheck_aspect"
        ]
      },
      "bzl": "zig/private/quality:zig.bzl",
      "file_extensions": [
        ".zig"
      ],
      "probes": {
        "format": [
          "real_zig_format_probe_aspect"
        ],
        "lint": [
          "real_zig_lint_probe_aspect"
        ],
        "typecheck": [
          "real_zig_typecheck_probe_aspect"
        ]
      }
    },
    "semver": false,
    "setup": {
      "hubs": [
        {
          "ecosystem": "zig",
          "extension": "@rules_dx//zig:extensions.bzl",
          "hub": "zig",
          "imports": "provides",
          "lock": "zig.lock.json",
          "manifest": "build.zig.zon",
          "resolver": "@rules_dx//zig/private/tools/lock",
          "state": null,
          "tag": "lock"
        }
      ]
    },
    "title": "Zig",
    "tools": [
      "zig_fmt"
    ]
  }
}""")
