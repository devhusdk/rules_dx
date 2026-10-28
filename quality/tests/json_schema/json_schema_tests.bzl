"""json_schema and json_schemas proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//quality:native_config.bzl", "DxNativeConfigInfo")

def _schemas_config_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[DxNativeConfigInfo]
    asserts.equals(env, "check_jsonschema", info.tool_id)
    asserts.equals(env, "schemas.schemas.json", info.config.basename)
    asserts.equals(
        env,
        [
            "schemas.json_schemas/quality/tests/json_schema/fixtures/a.schema.json",
            "schemas.json_schemas/quality/tests/json_schema/fixtures/common.schema.json",
            "schemas.schemas.json",
        ],
        sorted([f.short_path.removeprefix("quality/tests/json_schema/") for f in info.closure.to_list()]),
    )
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    config = json.decode(writes[0].content)
    asserts.equals(env, 1, config["schema_version"])
    asserts.equals(
        env,
        [
            {
                "schema": "schemas.json_schemas/quality/tests/json_schema/fixtures/a.schema.json",
                "url": "https://example.com/a.schema.json",
            },
            {
                "schema": "schemas.json_schemas/quality/tests/json_schema/fixtures/common.schema.json",
                "url": "https://example.com/common.schema.json",
            },
        ],
        config["resources"],
    )
    return analysistest.end(env)

schemas_config_test = analysistest.make(_schemas_config_impl)
