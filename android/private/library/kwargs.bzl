_SHARED_KWARGS = [
    "compatible_with",
    "deprecation",
    "exec_compatible_with",
    "features",
    "restricted_to",
    "tags",
    "target_compatible_with",
    "testonly",
]

def android_split_native_kwargs(kwargs):
    """Splits kwargs into attributes for every generated target and the rest."""
    shared = {key: kwargs[key] for key in _SHARED_KWARGS if key in kwargs}
    rest = {key: value for key, value in kwargs.items() if key not in shared}
    return shared, rest
