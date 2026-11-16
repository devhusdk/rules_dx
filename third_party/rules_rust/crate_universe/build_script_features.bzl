"""Rewrites crate_universe BUILD files so cargo_build_script targets keep coverage features off."""

NO_COVERAGE_FEATURES = ["-coverage", "-gcc_coverage_map_format", "-llvm_coverage_map_format"]

_CALL = "\ncargo_build_script(\n"

def build_scripts_without_coverage(text):
    """Adds `features = NO_COVERAGE_FEATURES` to each top-level `cargo_build_script(` call in `text`.

    Args:
        text: A crate_universe-rendered BUILD file.

    Returns:
        `text` with the features attribute on every cargo_build_script.
    """
    return text.replace(_CALL, _CALL + "    features = {},\n".format(json.encode(NO_COVERAGE_FEATURES)))
