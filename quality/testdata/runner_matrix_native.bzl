"""Native cohort matrix cells."""

NATIVE_CASES = [
    {
        "name": "matrix_c_format_pass",
        "generated": {
            "matrix/clang_format_clean.c": "// Seed C format fixture.\n#include <stdio.h>\n\nint greet(const char *name) {\n  return 0;\n}\n",
        },
        "capability": "format",
        "stages": ["clang_format;c;matrix/clang_format_clean.c"],
        "tool_names": ["clang_format"],
        "tool_binaries": ["//quality/testdata:fake_clang_format"],
        "expected_file": ":matrix/matrix_c_format_pass.expected.txt",
    },
    {
        "name": "matrix_c_format_fail",
        "generated": {
            "matrix/clang_format_dirty.c": "// Seed C format fixture.\n#include <stdio.h>\n\nint greet( const char*name){ BADFMT return 0; }\n",
        },
        "capability": "format",
        "stages": ["clang_format;c;matrix/clang_format_dirty.c"],
        "tool_names": ["clang_format"],
        "tool_binaries": ["//quality/testdata:fake_clang_format"],
        "expected_file": ":matrix/matrix_c_format_fail.expected.txt",
    },
    {
        "name": "matrix_cpp_format_pass",
        "generated": {
            "matrix/clang_format_clean.cpp": "// Seed C++ format fixture.\n\nint greet(const char *name) {\n  return 0;\n}\n",
        },
        "capability": "format",
        "stages": ["clang_format;cpp;matrix/clang_format_clean.cpp"],
        "tool_names": ["clang_format"],
        "tool_binaries": ["//quality/testdata:fake_clang_format"],
        "expected_file": ":matrix/matrix_cpp_format_pass.expected.txt",
    },
    {
        "name": "matrix_cpp_format_fail",
        "generated": {
            "matrix/clang_format_dirty.cpp": "// Seed C++ format fixture.\n\nint greet( const char*name){ BADFMT return 0; }\n",
        },
        "capability": "format",
        "stages": ["clang_format;cpp;matrix/clang_format_dirty.cpp"],
        "tool_names": ["clang_format"],
        "tool_binaries": ["//quality/testdata:fake_clang_format"],
        "expected_file": ":matrix/matrix_cpp_format_fail.expected.txt",
    },
    {
        "name": "matrix_c_lint_pass",
        "generated": {
            "matrix/clang_tidy_clean.c": "// Seed C lint fixture.\n#include <stdio.h>\n\nint check(int value) {\n  return value;\n}\n",
        },
        "capability": "lint",
        "stages": ["clang_tidy;c;matrix/clang_tidy_clean.c"],
        "upstream_tools": ["clang_tidy"],
        "upstream_srcs": [":matrix/matrix_c_lint_pass.upstream.clang_tidy.txt"],
        "expected_file": ":matrix/matrix_c_lint_pass.expected.txt",
    },
    {
        "name": "matrix_c_lint_fail",
        "generated": {
            "matrix/clang_tidy_dirty.c": "// Seed C lint fixture.\n#include <stdio.h>\n\nint check(int value) {\n  if (value) {\n    return 1;\n  } else {\n    return 0;\n  }\n}\n",
        },
        "capability": "lint",
        "stages": ["clang_tidy;c;matrix/clang_tidy_dirty.c"],
        "upstream_tools": ["clang_tidy"],
        "upstream_srcs": [":matrix/matrix_c_lint_fail.upstream.clang_tidy.txt"],
        "expected_file": ":matrix/matrix_c_lint_fail.expected.txt",
    },
    {
        "name": "matrix_cpp_lint_pass",
        "generated": {
            "matrix/cppcheck_clean.c": "// Seed C++ lint fixture.\n\nint check(int value) {\n  return value;\n}\n",
        },
        "capability": "lint",
        "stages": ["cppcheck;cpp;matrix/cppcheck_clean.c"],
        "upstream_tools": ["cppcheck"],
        "upstream_srcs": [":matrix/matrix_cpp_lint_pass.upstream.cppcheck.txt"],
        "expected_file": ":matrix/matrix_cpp_lint_pass.expected.txt",
    },
    {
        "name": "matrix_cpp_lint_fail",
        "generated": {
            "matrix/cppcheck_dirty.c": "// Seed C++ lint fixture.\nint check(int value) {\n  int *slot = nullptr;\n  if (value > 0) {\n    slot = &value;\n  }\n  return *slot;\n}\n",
        },
        "capability": "lint",
        "stages": ["cppcheck;cpp;matrix/cppcheck_dirty.c"],
        "upstream_tools": ["cppcheck"],
        "upstream_srcs": [":matrix/matrix_cpp_lint_fail.upstream.cppcheck.txt"],
        "expected_file": ":matrix/matrix_cpp_lint_fail.expected.txt",
    },
    {
        "name": "matrix_go_format_pass",
        "generated": {
            "matrix/gofumpt_clean.go": "// Seed Go format fixture.\npackage gofumpt\n\nfunc greet(name string) string {\n\treturn \"hello \" + name\n}\n",
        },
        "capability": "format",
        "stages": ["gofumpt;go;matrix/gofumpt_clean.go"],
        "tool_names": ["gofumpt"],
        "tool_binaries": ["//quality/testdata:fake_gofumpt"],
        "expected_file": ":matrix/matrix_go_format_pass.expected.txt",
    },
    {
        "name": "matrix_go_format_fail",
        "generated": {
            "matrix/gofumpt_dirty.go": "// Seed Go format fixture.\npackage gofumpt\n\nfunc greet( name string)string{ BADFMT return \"hello \"+name }\n",
        },
        "capability": "format",
        "stages": ["gofumpt;go;matrix/gofumpt_dirty.go"],
        "tool_names": ["gofumpt"],
        "tool_binaries": ["//quality/testdata:fake_gofumpt"],
        "expected_file": ":matrix/matrix_go_format_fail.expected.txt",
    },
    {
        "name": "matrix_go_staticcheck_pass",
        "generated": {
            "matrix/staticcheck_clean.go": "// Seed Go lint fixture.\npackage staticcheck\n\nfunc check(name string) string {\n\treturn \"hello \" + name\n}\n",
        },
        "capability": "lint",
        "stages": ["staticcheck;go;matrix/staticcheck_clean.go"],
        "upstream_tools": ["staticcheck"],
        "upstream_srcs": [":matrix/matrix_go_staticcheck_pass.upstream.staticcheck.txt"],
        "expected_file": ":matrix/matrix_go_staticcheck_pass.expected.txt",
    },
    {
        "name": "matrix_go_staticcheck_fail",
        "generated": {
            "matrix/staticcheck_dirty.go": "// Seed Go lint fixture.\npackage staticcheck\n\nfunc check(name string) string {\n\tx := 1\n\t_ = x\n\treturn \"hello \" + name\n}\n",
        },
        "capability": "lint",
        "stages": ["staticcheck;go;matrix/staticcheck_dirty.go"],
        "upstream_tools": ["staticcheck"],
        "upstream_srcs": [":matrix/matrix_go_staticcheck_fail.upstream.staticcheck.txt"],
        "expected_file": ":matrix/matrix_go_staticcheck_fail.expected.txt",
    },
    {
        "name": "matrix_go_govet_pass",
        "generated": {
            "matrix/govet_clean.go": "// Seed Go vet fixture.\npackage govet\n\nfunc check(name string) string {\n\treturn \"hello \" + name\n}\n",
        },
        "capability": "lint",
        "stages": ["govet;go;matrix/govet_clean.go"],
        "upstream_tools": ["govet"],
        "upstream_srcs": [":matrix/matrix_go_govet_pass.upstream.govet.txt"],
        "expected_file": ":matrix/matrix_go_govet_pass.expected.txt",
    },
    {
        "name": "matrix_go_govet_fail",
        "generated": {
            "matrix/govet_dirty.go": "// Seed Go vet fixture.\npackage govet\n\nimport \"fmt\"\n\nfunc check(name string) string {\n\tfmt.Printf(name)\n\treturn \"hello \" + name\n}\n",
        },
        "capability": "lint",
        "stages": ["govet;go;matrix/govet_dirty.go"],
        "upstream_tools": ["govet"],
        "upstream_srcs": [":matrix/matrix_go_govet_fail.upstream.govet.txt"],
        "expected_file": ":matrix/matrix_go_govet_fail.expected.txt",
    },
    {
        "name": "matrix_go_errcheck_pass",
        "generated": {
            "matrix/errcheck_clean.go": "// Seed Go errcheck fixture.\npackage errcheck\n\nfunc persist(path string) {\n}\n",
        },
        "capability": "lint",
        "stages": ["errcheck;go;matrix/errcheck_clean.go"],
        "upstream_tools": ["errcheck"],
        "upstream_srcs": [":matrix/matrix_go_errcheck_pass.upstream.errcheck.txt"],
        "expected_file": ":matrix/matrix_go_errcheck_pass.expected.txt",
    },
    {
        "name": "matrix_go_errcheck_fail",
        "generated": {
            "matrix/errcheck_dirty.go": "// Seed Go errcheck fixture.\npackage errcheck\n\nimport \"os\"\n\nfunc persist(path string) {\n\tf, _ := os.Create(path)\n\tf.WriteString(\"hello\")\n\tf.Close()\n}\n",
        },
        "capability": "lint",
        "stages": ["errcheck;go;matrix/errcheck_dirty.go"],
        "upstream_tools": ["errcheck"],
        "upstream_srcs": [":matrix/matrix_go_errcheck_fail.upstream.errcheck.txt"],
        "expected_file": ":matrix/matrix_go_errcheck_fail.expected.txt",
    },
]
