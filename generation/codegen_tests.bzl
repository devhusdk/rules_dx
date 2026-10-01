"""Unit and analysis tests for the normalized codegen plans."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(
    ":codegen.bzl",
    "CODEGEN_SCHEMA_VERSION",
    "DX_CODEGEN_ADMITTED_PAIRS",
    "DX_CODEGEN_PLAN_OUTPUT_GROUP",
    "DX_CODEGEN_SHARD_SUFFIX",
    "codegen_admitted_pairs",
    "codegen_conflict_error",
    "codegen_entry",
    "codegen_exec_error",
    "codegen_fingerprint_schema_error",
    "codegen_merge_records",
    "codegen_merge_schema_error",
    "codegen_pair_error",
    "codegen_path_error",
    "codegen_plan_fingerprint",
    "codegen_record",
    "codegen_record_error",
    "codegen_replaces_error",
    "codegen_schema_error",
)

def _record_a():
    return codegen_record(
        "//gen:alpha",
        "rust",
        [codegen_entry("src/alpha.rs", "src", "alpha")],
    )

def _record_b():
    return codegen_record(
        "//gen:beta",
        "rust",
        [codegen_entry("src/beta.rs", "src", "beta")],
    )

def codegen_defs_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "codegen schema version stays v1",
                CODEGEN_SCHEMA_VERSION,
                1,
            ),
            expect_equal(
                "codegen admitted-pair schema validates",
                codegen_schema_error(),
                "",
            ),
            expect_equal(
                "codegen freezes the private output group and shard suffix",
                [
                    DX_CODEGEN_PLAN_OUTPUT_GROUP,
                    DX_CODEGEN_SHARD_SUFFIX,
                ],
                ["dx_codegen_plans", ".dxcodegen.pb"],
            ),
            expect_equal(
                "codegen first pair stays admitted (additions need no allowlist edit)",
                codegen_pair_error("protobuf", "rust"),
                "",
            ),
            expect_equal(
                "codegen admitted query matches the registry data",
                codegen_admitted_pairs(),
                DX_CODEGEN_ADMITTED_PAIRS,
            ),
            expect_equal(
                "codegen_path_error accepts relative paths",
                [codegen_path_error("src/a.rs"), codegen_path_error("a")],
                ["", ""],
            ),
            expect_equal(
                "codegen_path_error rejects empty and absolute paths",
                [
                    codegen_path_error(""),
                    codegen_path_error("/src/a.rs"),
                    codegen_path_error("C:/src/a.rs"),
                ],
                [
                    "invalid codegen path '': must be a non-empty workspace-relative path",
                    "invalid codegen path '/src/a.rs': must not be absolute",
                    "invalid codegen path 'C:/src/a.rs': must not be absolute",
                ],
            ),
            expect_equal(
                "codegen_path_error keeps the drive rung inside the absolute one",
                [
                    codegen_path_error("C:src/a.rs"),
                    codegen_path_error("c:/src/a.rs"),
                    codegen_path_error("C:"),
                    codegen_path_error("C:\\src/a.rs"),
                    codegen_path_error("9:/src/a.rs"),
                    codegen_path_error("src/a:b"),
                    codegen_path_error("src/a.rs"),
                ],
                [
                    "invalid codegen path 'C:src/a.rs': must not be absolute",
                    "invalid codegen path 'c:/src/a.rs': must not be absolute",
                    "invalid codegen path 'C:': must not be absolute",
                    "invalid codegen path 'C:\\src/a.rs': must not be absolute",
                    "",
                    "",
                    "",
                ],
            ),
            expect_equal(
                "codegen_path_error rejects backslashes and dot segments",
                [
                    codegen_path_error("src\\a.rs"),
                    codegen_path_error("src/./a.rs"),
                    codegen_path_error("src/../a.rs"),
                ],
                [
                    "invalid codegen path 'src\\a.rs': must not contain '\\'",
                    "invalid codegen path 'src/./a.rs': must not contain '.' or '..' segments",
                    "invalid codegen path 'src/../a.rs': must not contain '.' or '..' segments",
                ],
            ),
            expect_equal(
                "codegen_entry marks projections read-only with empty exec and replaces by default",
                [codegen_entry("src/a.rs", "src", "a").read_only, codegen_entry("src/a.rs", "src", "a").exec_path, codegen_entry("src/a.rs", "src", "a").replaces],
                [True, "", ""],
            ),
            expect_equal(
                "codegen_exec_error accepts empty logical-only and relative suffixes",
                [codegen_exec_error(""), codegen_exec_error("result_proto.lib.rs"), codegen_exec_error("gen/out.rs")],
                ["", "", ""],
            ),
            expect_equal(
                "codegen_exec_error rejects bad suffixes",
                [
                    codegen_exec_error("/out/a.rs"),
                    codegen_exec_error("src\\a.rs"),
                    codegen_exec_error("src/./a.rs"),
                    codegen_exec_error("a.dxcodegen.pb"),
                ],
                [
                    "invalid codegen exec path '/out/a.rs': must not be absolute",
                    "invalid codegen exec path 'src\\a.rs': must not contain '\\'",
                    "invalid codegen exec path 'src/./a.rs': must not contain '.' or '..' segments",
                    "invalid codegen exec path 'a.dxcodegen.pb': must not use the reserved shard suffix '.dxcodegen.pb'",
                ],
            ),
            expect_equal(
                "codegen_record_error accepts a well-formed record",
                codegen_record_error(_record_a()),
                "",
            ),
            expect_equal(
                "codegen_record_error rejects bad producers",
                [
                    codegen_record_error(codegen_record("", "rust", [codegen_entry("a", "b")])),
                    codegen_record_error(codegen_record("gen:alpha", "rust", [codegen_entry("a", "b")])),
                ],
                [
                    "invalid codegen record: producer must be a non-empty label",
                    "invalid codegen record 'gen:alpha': producer must be a label in observation rendering",
                ],
            ),
            expect_equal(
                "codegen_record_error rejects empty language and empty entries",
                [
                    codegen_record_error(codegen_record("//gen:a", "", [codegen_entry("a", "b")])),
                    codegen_record_error(codegen_record("//gen:a", "rust", [])),
                ],
                [
                    "invalid codegen record '//gen:a': language must be a non-empty file class",
                    "invalid codegen record '//gen:a': entries must be non-empty (targets with no contribution carry no record)",
                ],
            ),
            expect_equal(
                "codegen_record_error rejects bad entry paths",
                codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("/a", "b")])),
                "invalid codegen record '//gen:a': invalid codegen path '/a': must not be absolute",
            ),
            expect_equal(
                "codegen_record_error rejects bad exec paths",
                codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "x.dxcodegen.pb")])),
                "invalid codegen record '//gen:a': invalid codegen exec path 'x.dxcodegen.pb': must not use the reserved shard suffix '.dxcodegen.pb'",
            ),
            expect_equal(
                "codegen_replaces_error accepts empty and self-equal contracted paths",
                [
                    codegen_replaces_error("a", "out/a.rs", ""),
                    codegen_replaces_error("a", "out/a.rs", "a"),
                ],
                ["", ""],
            ),
            expect_equal(
                "codegen_replaces_error rejects cross-path and exec-less contracts",
                [
                    codegen_replaces_error("a", "out/a.rs", "b"),
                    codegen_replaces_error("a", "", "a"),
                    codegen_replaces_error("a", "out/a.rs", "/a"),
                ],
                [
                    "invalid codegen replaces 'b': must equal logical path 'a'",
                    "invalid codegen replaces 'a': needs a non-empty exec path binding the replacing artifact",
                    "invalid codegen replaces '/a': must not be absolute",
                ],
            ),
            expect_equal(
                "codegen_record_error accepts an explicit replacement contract",
                codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")])),
                "",
            ),
            expect_equal(
                "codegen_record_error rejects bad replacement contracts",
                [
                    codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "", "a")])),
                    codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "b")])),
                ],
                [
                    "invalid codegen record '//gen:a': invalid codegen replaces 'a': needs a non-empty exec path binding the replacing artifact",
                    "invalid codegen record '//gen:a': invalid codegen replaces 'b': must equal logical path 'a'",
                ],
            ),
            expect_equal(
                "codegen_record_error rejects within-record duplicate paths",
                codegen_record_error(codegen_record("//gen:a", "rust", [codegen_entry("a", "b"), codegen_entry("a", "c")])),
                "invalid codegen record '//gen:a': duplicate logical path 'a'",
            ),
            expect_equal(
                "codegen_conflict_error accepts disjoint records",
                codegen_conflict_error([_record_a(), _record_b()]),
                "",
            ),
            expect_equal(
                "codegen_conflict_error merges identical duplicates silently",
                codegen_conflict_error([_record_a(), _record_a()]),
                "",
            ),
            expect_equal(
                "codegen_conflict_error fails cross-producer collisions",
                codegen_conflict_error([
                    _record_a(),
                    codegen_record("//gen:evil", "rust", [codegen_entry("src/alpha.rs", "src", "alpha")]),
                ]),
                "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:evil",
            ),
            expect_equal(
                "codegen_conflict_error fails divergent roots from one producer",
                codegen_conflict_error([
                    _record_a(),
                    codegen_record("//gen:alpha", "rust", [codegen_entry("src/alpha.rs", "other", "alpha")]),
                ]),
                "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:alpha",
            ),
            expect_equal(
                "codegen_conflict_error fails divergent exec paths",
                codegen_conflict_error([
                    _record_a(),
                    codegen_record("//gen:alpha", "rust", [codegen_entry("src/alpha.rs", "src", "alpha", "out/a.rs")]),
                ]),
                "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:alpha",
            ),
            expect_equal(
                "codegen_conflict_error merges identical replacement contracts silently",
                codegen_conflict_error([
                    codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")]),
                    codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")]),
                ]),
                "",
            ),
            expect_equal(
                "codegen_conflict_error fails divergent replacement contracts",
                codegen_conflict_error([
                    codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs")]),
                    codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")]),
                ]),
                "codegen path conflict: logical path 'a' claimed by //gen:a, //gen:a",
            ),
            expect_equal(
                "codegen_merge_records normalizes without pinning exact contents (schema)",
                codegen_merge_schema_error(
                    [
                        codegen_record("//gen:b", "rust", [codegen_entry("z.rs", "s"), codegen_entry("a.rs", "s")]),
                        _record_a(),
                        _record_a(),
                        codegen_record("//gen:a", "python", [codegen_entry("m.py", "s")]),
                    ],
                    codegen_merge_records([
                        codegen_record("//gen:b", "rust", [codegen_entry("z.rs", "s"), codegen_entry("a.rs", "s")]),
                        _record_a(),
                        _record_a(),
                        codegen_record("//gen:a", "python", [codegen_entry("m.py", "s")]),
                    ]),
                ),
                "",
            ),
            expect_equal(
                "codegen_merge_records groups by owner and sorts entries (snapshot)",
                [
                    (record.producer, record.language, [entry.logical_path for entry in record.entries])
                    for record in codegen_merge_records([
                        codegen_record("//gen:b", "rust", [codegen_entry("z.rs", "s"), codegen_entry("a.rs", "s")]),
                        _record_a(),
                        _record_a(),
                        codegen_record("//gen:a", "python", [codegen_entry("m.py", "s")]),
                    ])
                ],
                [
                    ("//gen:a", "python", ["m.py"]),
                    ("//gen:alpha", "rust", ["src/alpha.rs"]),
                    ("//gen:b", "rust", ["a.rs", "z.rs"]),
                ],
            ),
            expect_equal(
                "codegen_plan_fingerprint validates hash-input shape (schema)",
                codegen_fingerprint_schema_error(codegen_plan_fingerprint([_record_b(), _record_a(), _record_a()])),
                "",
            ),
            expect_equal(
                "codegen_plan_fingerprint renders the normalized hash input (snapshot)",
                codegen_plan_fingerprint([_record_b(), _record_a(), _record_a()]),
                "[{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"src\",\"logical_path\":\"src/alpha.rs\",\"namespace\":\"alpha\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:alpha\"}," +
                "{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"src\",\"logical_path\":\"src/beta.rs\",\"namespace\":\"beta\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:beta\"}]",
            ),
            expect_equal(
                "codegen_plan_fingerprint exec paths validate shape (schema)",
                codegen_fingerprint_schema_error(codegen_plan_fingerprint([codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs")])])),
                "",
            ),
            expect_equal(
                "codegen_plan_fingerprint binds exec paths (snapshot)",
                codegen_plan_fingerprint([codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs")])]),
                "[{\"entries\":[{\"exec_path\":\"out/a.rs\",\"import_root\":\"b\",\"logical_path\":\"a\",\"namespace\":\"\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:a\"}]",
            ),
            expect_equal(
                "codegen_plan_fingerprint binds the replacement contract (snapshot)",
                codegen_plan_fingerprint([codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")])]),
                "[{\"entries\":[{\"exec_path\":\"out/a.rs\",\"import_root\":\"b\",\"logical_path\":\"a\",\"namespace\":\"\",\"read_only\":true,\"replaces\":\"a\"}],\"language\":\"rust\",\"producer\":\"//gen:a\"}]",
            ),
            expect_equal(
                "codegen_plan_fingerprint contracted entries validate shape (schema)",
                codegen_fingerprint_schema_error(codegen_plan_fingerprint([codegen_record("//gen:a", "rust", [codegen_entry("a", "b", "", "out/a.rs", "a")])])),
                "",
            ),
            expect_equal(
                "codegen_pair_error defers non-admitted pairs (message lists the registry data)",
                [
                    codegen_pair_error("protobuf", "rust") == "",
                    codegen_pair_error("graphql", "rust") == "",
                    codegen_pair_error("protobuf", "python") == "",
                ],
                [
                    True,
                    False,
                    False,
                ],
            ),
        ],
    )

_CHAIN_FINGERPRINT = (
    "[{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"gen\",\"logical_path\":\"gen/alpha.rs\"," +
    "\"namespace\":\"alpha\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\"," +
    "\"producer\":\"//generation:codegen_shard_alpha\"}," +
    "{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"gen\",\"logical_path\":\"gen/beta.rs\"," +
    "\"namespace\":\"beta\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\"," +
    "\"producer\":\"//generation:codegen_shard_beta\"}]"
)

_PROST_FINGERPRINT = (
    "[{\"entries\":[{\"exec_path\":\"result_proto.lib.rs\",\"import_root\":\"gen\",\"logical_path\":\"gen/prost_result.rs\"," +
    "\"namespace\":\"result\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\"," +
    "\"producer\":\"//generation:codegen_prost_fixture\"}]"
)

EXPECTED_CODEGEN_PLAN_OBSERVATIONS = """subject //generation:codegen_plan_chain_subject
field files=codegen_shard_alpha.dxcodegen.pb,codegen_shard_beta.dxcodegen.pb
field fingerprint=%s
field label=//generation:codegen_shard_beta
field record_count=2
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//generation:codegen_plan_chain_subject
aspect_field transitive_count=0
subject //generation:codegen_plan_prost_subject
field files=codegen_prost_fixture.dxcodegen.pb,result_proto.lib.rs
field fingerprint=%s
field label=//generation:codegen_prost_fixture
field record_count=1
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//generation:codegen_plan_prost_subject
aspect_field transitive_count=0""" % (_CHAIN_FINGERPRINT, _PROST_FINGERPRINT)

def codegen_plan_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":codegen_plan_chain_subject",
            ":codegen_plan_prost_subject",
        ],
        expected_observations = EXPECTED_CODEGEN_PLAN_OBSERVATIONS,
    )
