"""F# compile symbols resolved at analysis time."""

load("@rules_dotnet//dotnet/private:common.bzl", "framework_preprocessor_symbols", "get_highest_compatible_target_framework")

def fsharp_compile_defines(incoming_tfm, target_frameworks, defines):
    """Returns the framework symbols of the compiled TFM plus the user defines."""
    if not target_frameworks:
        return list(defines)
    tfm = get_highest_compatible_target_framework(incoming_tfm, target_frameworks)
    if tfm == None:
        return list(defines)
    return framework_preprocessor_symbols(tfm) + list(defines)
