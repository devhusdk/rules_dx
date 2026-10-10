"""Execution tests over runfiles fixtures."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_match", "expect_true", "starlark_harness_test")

def fixture_execution_tests(name):
    starlark_harness_test(
        name = name,
        checks = [
            expect_equal("record encoding is deterministic", expect_equal("x", 1, 2), "{\"actual\":1,\"expected\":2,\"kind\":\"equal\",\"name\":\"x\"}"),
            expect_equal("multiline value", "first\nsecond", "first\nsecond"),
            expect_equal("quoted value", "say \"hi\" `now` $HOME \\ done", "say \"hi\" `now` $HOME \\ done"),
            expect_equal("name with \"quotes\" and $dollar", 1, 1),
            expect_true("one equals one", 1 == 1),
            expect_false("one equals two", 1 == 2),
            expect_contains("string contains word", "hello world", "o w"),
            expect_contains("list contains element", [1, 2, 3], 2),
            expect_contains("dict contains key", {"a": 1}, "a"),
            expect_match("match substring", {"a": 1}, "a"),
        ],
        file_checks = {
            ":answer_fixture.txt": "answer=42\nnote \"quoted\" $HOME `tap` \\ done",
            ":shape_fixture.txt": "shape=circle",
            ":spaced name.txt": "spaced needle here",
        },
    )
