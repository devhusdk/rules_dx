"""Matrix cases: every tier file is already formatted, so a format run changes nothing."""

load("//quality/tiers:corpora.bzl", "TIER_JAVASCRIPT", "TIER_JSON", "TIER_TOML", "TIER_YAML", "tier_labels")

def _fixpoint_case(name, tool, binary, source_class, paths, config = None):
    sources = ",".join(["quality/tiers/" + path for path in paths])
    case = {
        "name": name,
        "srcs": tier_labels(paths),
        "capability": "format",
        "stages": ["%s;%s;%s" % (tool, source_class, sources)],
        "tool_names": [tool],
        "tool_binaries": [binary],
        "expected": "\n".join(
[
            "producer //quality/testdata:" + name,
            "capability FORMAT",
            "stages 1",
            "stage %s classes=%s sources=%s" % (tool, source_class, sources),
            "completed_rounds 1",
            "convergence STABLE",
            "initial 0",
            "terminal 0",
            "replacements 0",
            "",
        ]
        ),
    }
    if config:
        case["toolfile_tools"] = [tool]
        case["toolfile_srcs"] = [config]
        case["config_tools"] = [tool]
        case["config_files"] = [config]
    return case

TIERS_CASES = [
    _fixpoint_case("matrix_tiers_javascript_format_pass", "biome", "@dx_tools//:biome", "javascript", TIER_JAVASCRIPT, "//:biome.json"),
    _fixpoint_case("matrix_tiers_json_format_pass", "prettier", "//quality/tools/javascript/bin:prettier", "json", TIER_JSON),
    _fixpoint_case("matrix_tiers_toml_format_pass", "taplo", "@dx_tools//:taplo", "toml", TIER_TOML),
    _fixpoint_case("matrix_tiers_yaml_format_pass", "yamlfmt", "@dx_tools//:yamlfmt", "yaml", TIER_YAML),
]
