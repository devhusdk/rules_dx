"""Generated from every <lang>/lang.json by //core/modules:lang_json_test; refresh with UPDATE_EXPECT=1."""

LANGS = json.decode("""{
  "android": {
    "ecosystems": [],
    "entry_points": [
      "java_android_library",
      "kotlin_android_library",
      "android_native_library",
      "rust_android_apk",
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
    "platforms": [],
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
    "lockfiles": [],
    "manifests": [
      "MODULE.bazel"
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
      "file_extensions": [
        ".yaml",
        ".yml"
      ]
    },
    "title": "ansible",
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
      "api_spec_library"
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
    "title": "api_spec",
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
      "apple_provisioning",
      "apple_toolchain_probe",
      "apple_simulator_boot",
      "apple_simulator_install",
      "apple_simulator_launch"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "apple",
    "pins": [],
    "platforms": [],
    "title": "Apple",
    "tools": []
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
    "title": "asciidoc",
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
      "avro_library"
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
    "lockfiles": [],
    "manifests": [
      "MODULE.bazel"
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
    "title": "bicep",
    "tools": [
      "bicep",
      "trivy_config"
    ]
  },
  "capnproto": {
    "ecosystems": [],
    "entry_points": [
      "capnp_library"
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
    "title": "capnproto",
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
    "ecosystems": [
      "conan"
    ],
    "entry_points": [
      "cc_coverage_settings",
      "cc_library",
      "objc_library",
      "cc_binary",
      "cc_test",
      "cc_fuzz_test",
      "cc_bench"
    ],
    "family": "core",
    "lockfiles": [
      "conan.lock"
    ],
    "manifests": [
      "conanfile.txt"
    ],
    "name": "cc",
    "pins": [
      "private/toolchains/pins.bzl"
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
      "file_extensions": [
        ".yaml",
        ".yml",
        ".json"
      ]
    },
    "title": "cloudformation",
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
      ]
    },
    "title": "cmake",
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
      "cuda_component_library",
      "cuda_gpu_env"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "cuda",
    "pins": [],
    "platforms": [],
    "title": "CUDA",
    "tools": []
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
      "private/toolchain/pins.json"
    ],
    "platforms": [],
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
    "title": "D",
    "tools": []
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
      "dart_bench"
    ],
    "family": "core",
    "lockfiles": [
      "pubspec.lock"
    ],
    "manifests": [
      "pubspec.yaml"
    ],
    "name": "dart",
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
    "title": "Dart",
    "tools": [
      "dart"
    ]
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
      "file_extensions": []
    },
    "title": "dockerfile",
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
    "pins": [],
    "platforms": [],
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
      "bzl": "core/quality:families/dotnet.bzl",
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
    "title": "dotnet",
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
      "escript_archive",
      "escript_headers",
      "elixir_app",
      "elixir_test",
      "elixir_bench"
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
      "private/toolchain/pins.json"
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
      "bzl": "core/quality:families/beam.bzl",
      "file_extensions": [
        ".ex",
        ".exs"
      ]
    },
    "semver": false,
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
      "erlc_opts",
      "erlang_xrl_yrl",
      "erlang_nif",
      "erlang_release",
      "erlang_app",
      "erlang_test_app",
      "erlang_bytecode",
      "erlang_escript_archive",
      "erlang_eunit_test",
      "erlang_ct_suite_test",
      "erlang_bench"
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
      "private/toolchain/pins.json"
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
      "bzl": "core/quality:families/beam.bzl",
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
      "flatbuffers_library"
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
    "title": "flatbuffers",
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
    "pins": [],
    "platforms": [],
    "title": "Flutter",
    "tools": []
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
    "lockfiles": [],
    "manifests": [
      "fpm.toml"
    ],
    "name": "fortran",
    "pins": [
      "private/toolchain/pins.json"
    ],
    "platforms": [
      "linux_x86_64"
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
    "title": "Fortran",
    "tools": [
      "ffmt",
      "fortitude"
    ]
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
      "haskell_bench"
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
      "private/toolchain/pins.bzl",
      "private/toolchain/pins.json"
    ],
    "platforms": [
      "linux_arm64",
      "linux_x86_64",
      "macos_arm64"
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
    "title": "Haskell",
    "tools": [
      "hlint",
      "ormolu",
      "weeder"
    ]
  },
  "helm": {
    "ecosystems": [],
    "entry_points": [
      "helm_chart"
    ],
    "family": "core",
    "lockfiles": [
      "Chart.lock"
    ],
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
    "title": "helm",
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
      "java_jmh_test",
      "java_fuzz_test",
      "java_bench"
    ],
    "family": "core",
    "lockfiles": [
      "maven_install.json"
    ],
    "manifests": [
      "MODULE.bazel"
    ],
    "name": "java",
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
          "real_jvm_format_aspect"
        ],
        "lint": [
          "real_jvm_lint_aspect"
        ],
        "typecheck": [
          "real_jvm_typecheck_aspect"
        ]
      },
      "bzl": "core/quality:families/jvm.bzl",
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
      "javascript_fuzz_test"
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
      "julia_standalone_binary"
    ],
    "family": "core",
    "lockfiles": [
      "Manifest.toml"
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
    "title": "Julia",
    "tools": [
      "julia_format",
      "julialint"
    ]
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
      "kotlin_warnings_as_errors_opts",
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
      "MODULE.bazel"
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
      "bzl": "core/quality:families/jvm.bzl",
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
      "file_extensions": [
        ".yaml",
        ".yml"
      ]
    },
    "title": "kubernetes",
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
      "lua_test"
    ],
    "family": "core",
    "lockfiles": [
      "luarocks.lock"
    ],
    "manifests": [
      "*.rockspec"
    ],
    "name": "lua",
    "pins": [
      "private/toolchain/pins.json"
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
      "linux_x86_64"
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
      ]
    },
    "title": "make",
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
      "private/pins.bzl"
    ],
    "platforms": [
      "windows_arm64",
      "windows_x86_64"
    ],
    "title": "MSIX",
    "tools": []
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
      "ocamllex",
      "ocaml_module",
      "ocaml_signature",
      "ocaml_library",
      "ocaml_binary",
      "ocaml_test",
      "ocaml_fuzz_test",
      "ocaml_bench",
      "dx_dune_library"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [
      "dune-project",
      "*.opam"
    ],
    "name": "ocaml",
    "pins": [
      "private/toolchain/pins.bzl",
      "private/toolchain/pins.json"
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
    "title": "OCaml",
    "tools": [
      "topiary",
      "zanuda"
    ]
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
    "title": "Perl",
    "tools": [
      "perlcritic",
      "perltidy"
    ]
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
      "private/toolchain/pins.json"
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
    "title": "PHP",
    "tools": [
      "phpcs",
      "phpstan",
      "prettier"
    ]
  },
  "pkl": {
    "ecosystems": [],
    "entry_points": [],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "pkl",
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
          "real_pkl_format_aspect"
        ],
        "lint": [
          "real_pkl_lint_aspect"
        ],
        "typecheck": [
          "real_pkl_typecheck_aspect"
        ]
      },
      "bzl": "pkl/private/quality:pkl.bzl",
      "file_extensions": [
        ".pkl"
      ],
      "probes": {
        "format": [
          "real_pkl_format_probe_aspect"
        ],
        "lint": [
          "real_pkl_lint_probe_aspect"
        ],
        "typecheck": [
          "real_pkl_typecheck_probe_aspect"
        ]
      }
    },
    "title": "pkl",
    "tools": [
      "pkl"
    ]
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
      "PSGallery.lock.json"
    ],
    "manifests": [
      "PSGallery.requirements.psd1"
    ],
    "name": "powershell",
    "pins": [
      "private/toolchain/pins.json"
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
      "qt_bundle"
    ],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "qt",
    "pins": [
      "private/sdk/pins.bzl"
    ],
    "platforms": [],
    "quality": {
      "bzl": "core/quality:families/cc.bzl",
      "file_extensions": [
        ".qml"
      ]
    },
    "title": "qt",
    "tools": []
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
    "platforms": [],
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
    "title": "rego",
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
    "title": "rst",
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
      "private/toolchain/pins.json"
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
      "rust_fuzz_test"
    ],
    "family": "core",
    "lockfiles": [
      "Cargo.lock"
    ],
    "manifests": [
      "Cargo.toml"
    ],
    "name": "rust",
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
      "MODULE.bazel"
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
      "bzl": "core/quality:families/jvm.bzl",
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
    "title": "sql",
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
    "pins": [],
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
    "title": "Swift",
    "tools": [
      "swift_format",
      "swift_lint"
    ]
  },
  "terraform": {
    "ecosystems": [],
    "entry_points": [
      "terraform_library"
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
    "title": "terraform",
    "tools": [
      "terraform",
      "tflint",
      "trivy_config"
    ]
  },
  "thrift": {
    "ecosystems": [],
    "entry_points": [
      "thrift_library"
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
    "title": "thrift",
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
      "package.json",
      "tsconfig.json"
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
  "wasi": {
    "ecosystems": [],
    "entry_points": [],
    "family": "core",
    "lockfiles": [],
    "manifests": [],
    "name": "wasi",
    "pins": [],
    "platforms": [],
    "title": "wasi",
    "tools": []
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
    "title": "Zig",
    "tools": [
      "zig_fmt"
    ]
  }
}""")
