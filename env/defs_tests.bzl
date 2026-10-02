"""Unit and analysis tests for the bootstrap environment registry."""

load("//libs/starlark:defs.bzl", "display_label", "expect_equal", "starlark_test")
load(
    ":defs.bzl",
    "env_collision_error",
    "env_host_filename",
    "env_name_error",
    "env_tool_error",
    "env_tool_record",
    "env_tree_metadata",
)

def env_defs_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "env_name_error accepts a dotted, dashed tool name",
                env_name_error("my-tool.cli"),
                "",
            ),
            expect_equal(
                "env_name_error rejects an empty name",
                env_name_error(""),
                "invalid host name '': must be a non-empty single path component",
            ),
            expect_equal(
                "env_name_error rejects dot segments",
                [env_name_error("."), env_name_error("..")],
                [
                    "invalid host name '.': must not be '.' or '..'",
                    "invalid host name '..': must not be '.' or '..'",
                ],
            ),
            expect_equal(
                "env_name_error rejects path separators",
                [env_name_error("a/b"), env_name_error("a\\b")],
                [
                    "invalid host name 'a/b': must not contain '/' or '\\'",
                    "invalid host name 'a\\b': must not contain '/' or '\\'",
                ],
            ),
            expect_equal(
                "env_name_error rejects executable suffixes case-insensitively",
                [env_name_error("tool.exe"), env_name_error("tool.BAT")],
                [
                    "invalid host name 'tool.exe': must not carry an executable suffix (found '.exe')",
                    "invalid host name 'tool.BAT': must not carry an executable suffix (found '.bat')",
                ],
            ),
            expect_equal(
                "env_name_error rejects Windows reserved stems with extensions",
                [env_name_error("CON"), env_name_error("com1"), env_name_error("nul.txt")],
                [
                    "invalid host name 'CON': stem 'con' is reserved on Windows",
                    "invalid host name 'com1': stem 'com1' is reserved on Windows",
                    "invalid host name 'nul.txt': stem 'nul' is reserved on Windows",
                ],
            ),
            expect_equal(
                "env_name_error rejects the reserved provenance marker name",
                [env_name_error(".rules_dx_managed"), env_name_error(".RULES_DX_MANAGED")],
                [
                    "invalid host name '.rules_dx_managed': is reserved for the provenance marker",
                    "invalid host name '.RULES_DX_MANAGED': is reserved for the provenance marker",
                ],
            ),
            expect_equal(
                "env_tool_error accepts a record with aliases",
                env_tool_error("alpha", ["a"]),
                "",
            ),
            expect_equal(
                "env_tool_error rejects a reserved alias",
                env_tool_error("alpha", [".rules_dx_managed"]),
                "invalid alias '.rules_dx_managed': invalid host name '.rules_dx_managed': is reserved for the provenance marker",
            ),
            expect_equal(
                "env_tool_error rejects a bad primary name",
                env_tool_error("", []),
                "invalid bin_name: invalid host name '': must be a non-empty single path component",
            ),
            expect_equal(
                "env_tool_error rejects a bad alias",
                env_tool_error("alpha", ["good", "bad/x"]),
                "invalid alias 'bad/x': invalid host name 'bad/x': must not contain '/' or '\\'",
            ),
            expect_equal(
                "display_label strips the canonical marker from main-repo labels",
                display_label(Label("//env:tool_alpha")),
                "//env:tool_alpha",
            ),
            expect_equal(
                "display_label passes marker-free labels through",
                display_label("//already/plain"),
                "//already/plain",
            ),
            expect_equal(
                "env_collision_error accepts disjoint records",
                env_collision_error([
                    env_tool_record("//env:tool_alpha", "alpha", ["a"]),
                    env_tool_record("//env:tool_beta", "beta", []),
                ]),
                "",
            ),
            expect_equal(
                "env_collision_error ignores same-owner repeats",
                env_collision_error([
                    env_tool_record("//env:tool_alpha", "alpha", ["alpha", "a"]),
                ]),
                "",
            ),
            expect_equal(
                "env_collision_error reports a primary/alias cross-tool collision",
                env_collision_error([
                    env_tool_record("//env:tool_alpha", "alpha", ["a"]),
                    env_tool_record("//env:tool_beta", "beta", ["a"]),
                ]),
                "host-name collision: host name 'a' claimed by //env:tool_alpha, //env:tool_beta",
            ),
            expect_equal(
                "env_collision_error folds case across owners",
                env_collision_error([
                    env_tool_record("//env:tool_alpha", "Alpha", []),
                    env_tool_record("//env:tool_beta", "ALPHA", []),
                ]),
                "host-name collision: host name 'alpha' claimed by //env:tool_alpha, //env:tool_beta",
            ),
            expect_equal(
                "env_collision_error lists every collision sorted",
                env_collision_error([
                    env_tool_record("//env:tool_alpha", "shared", ["z-shared"]),
                    env_tool_record("//env:tool_beta", "beta", ["Shared"]),
                    env_tool_record("//env:tool_gamma", "gamma", ["Z-Shared"]),
                ]),
                "host-name collision: host name 'shared' claimed by //env:tool_alpha, //env:tool_beta; " +
                "host name 'z-shared' claimed by //env:tool_alpha, //env:tool_gamma",
            ),
            expect_equal(
                "env_host_filename keeps logical names on POSIX",
                [env_host_filename("alpha", False), env_host_filename("my-tool.cli", False)],
                ["alpha", "my-tool.cli"],
            ),
            expect_equal(
                "env_host_filename appends .exe on Windows",
                [env_host_filename("alpha", True), env_host_filename("my-tool.cli", True)],
                ["alpha.exe", "my-tool.cli.exe"],
            ),
            expect_equal(
                "env_tree_metadata versions and sorts the tool entries",
                json.decode(env_tree_metadata(
                    [
                        env_tool_record("//env:tool_beta", "beta", []),
                        env_tool_record("//env:tool_alpha", "alpha", ["a"]),
                    ],
                    False,
                )),
                {
                    "schema_version": 1,
                    "tools": [
                        {
                            "aliases": ["a"],
                            "bin_name": "alpha",
                            "host_names": ["alpha", "a"],
                            "owner": "//env:tool_alpha",
                        },
                        {
                            "aliases": [],
                            "bin_name": "beta",
                            "host_names": ["beta"],
                            "owner": "//env:tool_beta",
                        },
                    ],
                },
            ),
            expect_equal(
                "env_tree_metadata maps host names on Windows",
                json.decode(env_tree_metadata(
                    [env_tool_record("//env:tool_alpha", "alpha", ["a"])],
                    True,
                ))["tools"],
                [
                    {
                        "aliases": ["a"],
                        "bin_name": "alpha",
                        "host_names": ["alpha.exe", "a.exe"],
                        "owner": "//env:tool_alpha",
                    },
                ],
            ),
        ],
    )

EXPECTED_ENV_OBSERVATIONS = """subject //env:config_under_test
field count=2
field names=a,alpha,beta
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//env:config_under_test
aspect_field transitive_count=0"""

def env_config_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":config_under_test"],
        expected_observations = EXPECTED_ENV_OBSERVATIONS,
    )

EXPECTED_TREE_OBSERVATIONS = """subject //env:tree_under_test
file a
file alpha
file beta
file tree_under_test.metadata.json
field count=2
field host_names=a,alpha,beta
field platform=posix
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//env:tree_under_test
aspect_field transitive_count=0"""

WINDOWS_TREE_OBSERVATIONS = """subject //env:tree_under_test
file a
file alpha
file beta
file tree_under_test.metadata.json
field count=2
field host_names=a.exe,alpha.exe,beta.exe
field platform=windows
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//env:tree_under_test
aspect_field transitive_count=0"""

def env_tree_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":tree_under_test"],
        expected_observations = EXPECTED_TREE_OBSERVATIONS,
        windows_expected_observations = WINDOWS_TREE_OBSERVATIONS,
    )

EXPECTED_DEFAULT_CONFIG_OBSERVATIONS = """subject //env:default_config
field count=3
field names=doctor,dx,quality_markdown
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//env:default_config
aspect_field transitive_count=0"""

def env_default_config_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":default_config"],
        expected_observations = EXPECTED_DEFAULT_CONFIG_OBSERVATIONS,
    )

EXPECTED_DEFAULT_TREE_OBSERVATIONS = """subject //env:default_tree
file default_tree.metadata.json
file doctor
file dx
file quality_markdown
field count=3
field host_names=doctor,dx,quality_markdown
field platform=posix
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//env:default_tree
aspect_field transitive_count=0"""

WINDOWS_DEFAULT_TREE_OBSERVATIONS = """subject //env:default_tree
file default_tree.metadata.json
file doctor
file dx
file quality_markdown
field count=3
field host_names=doctor.exe,dx.exe,quality_markdown.exe
field platform=windows
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//env:default_tree
aspect_field transitive_count=0"""

def env_default_tree_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":default_tree"],
        expected_observations = EXPECTED_DEFAULT_TREE_OBSERVATIONS,
        windows_expected_observations = WINDOWS_DEFAULT_TREE_OBSERVATIONS,
    )
