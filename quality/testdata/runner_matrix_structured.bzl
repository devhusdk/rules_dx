"""Structured cohort matrix cells."""

STRUCTURED_CASES = [
    {
        "name": "matrix_protobuf_format_pass",
        "generated": {
            "matrix/buf_clean.proto": "syntax = \"proto3\";\n\npackage fixtures.buf;\n\nmessage Sample {\n  string greet = 1;\n}\n",
        },
        "capability": "format",
        "stages": ["buf;protobuf;matrix/buf_clean.proto"],
        "tool_names": ["buf"],
        "tool_binaries": ["//quality/testdata:fake_buf_format"],
        "expected_file": ":matrix/matrix_protobuf_format_pass.expected.txt",
    },
    {
        "name": "matrix_protobuf_format_fail",
        "generated": {
            "matrix/buf_dirty.proto": "syntax = \"proto3\";\n\npackage fixtures.buf; BADFMT\n",
        },
        "capability": "format",
        "stages": ["buf;protobuf;matrix/buf_dirty.proto"],
        "tool_names": ["buf"],
        "tool_binaries": ["//quality/testdata:fake_buf_format"],
        "expected_file": ":matrix/matrix_protobuf_format_fail.expected.txt",
    },
    {
        "name": "matrix_protobuf_lint_pass",
        "generated": {
            "matrix/buf_lint_clean.proto": "syntax = \"proto3\";\n\npackage fixtures.buf;\n\nmessage Sample {\n  string greet = 1;\n}\n",
        },
        "capability": "lint",
        "stages": ["buf;protobuf;matrix/buf_lint_clean.proto"],
        "upstream_tools": ["buf"],
        "upstream_srcs": [":matrix/matrix_protobuf_lint_pass.upstream.buf.txt"],
        "expected_file": ":matrix/matrix_protobuf_lint_pass.expected.txt",
    },
    {
        "name": "matrix_protobuf_lint_fail",
        "generated": {
            "matrix/buf_lint_dirty.proto": "syntax = \"proto3\";\n\npackage fixtures.buf;\n\nmessage Sample {\n  string greet = 1;\n}\n",
        },
        "capability": "lint",
        "stages": ["buf;protobuf;matrix/buf_lint_dirty.proto"],
        "upstream_tools": ["buf"],
        "upstream_srcs": [":matrix/matrix_protobuf_lint_fail.upstream.buf.txt"],
        "expected_file": ":matrix/matrix_protobuf_lint_fail.expected.txt",
    },
    {
        "name": "matrix_qml_format_pass",
        "generated": {
            "matrix/qmlformat_clean.qml": "import QtQuick\n\nItem {\n    property string greet: \"hello\"\n}\n",
        },
        "capability": "format",
        "stages": ["qmlformat;qml;matrix/qmlformat_clean.qml"],
        "tool_names": ["qmlformat"],
        "tool_binaries": ["//quality/testdata:fake_qmlformat"],
        "expected_file": ":matrix/matrix_qml_format_pass.expected.txt",
    },
    {
        "name": "matrix_qml_format_fail",
        "generated": {
            "matrix/qmlformat_dirty.qml": "import QtQuick\n\nItem { BADFMT }\n",
        },
        "capability": "format",
        "stages": ["qmlformat;qml;matrix/qmlformat_dirty.qml"],
        "tool_names": ["qmlformat"],
        "tool_binaries": ["//quality/testdata:fake_qmlformat"],
        "expected_file": ":matrix/matrix_qml_format_fail.expected.txt",
    },
    {
        "name": "matrix_qml_lint_pass",
        "generated": {
            "matrix/qmllint_clean.qml": "import QtQuick\n\nItem {\n    property string greet: \"hello\"\n}\n",
        },
        "capability": "lint",
        "stages": ["qmllint;qml;matrix/qmllint_clean.qml"],
        "upstream_tools": ["qmllint"],
        "upstream_srcs": [":matrix/matrix_qml_lint_pass.upstream.qmllint.txt"],
        "expected_file": ":matrix/matrix_qml_lint_pass.expected.txt",
    },
    {
        "name": "matrix_qml_lint_fail",
        "generated": {
            "matrix/qmllint_dirty.qml": "import QtQuick\n\nItem {\n    property string greet: foo\n}\n",
        },
        "capability": "lint",
        "stages": ["qmllint;qml;matrix/qmllint_dirty.qml"],
        "upstream_tools": ["qmllint"],
        "upstream_srcs": [":matrix/matrix_qml_lint_fail.upstream.qmllint.txt"],
        "expected_file": ":matrix/matrix_qml_lint_fail.expected.txt",
    },
]
