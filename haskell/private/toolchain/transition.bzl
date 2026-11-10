"""Configuration that lets Haskell targets resolve the bundled mingw C toolchain."""

_FLAG = str(Label("//haskell/private/toolchain:ghc_mingw"))

def _impl(_settings, _attr):
    return {_FLAG: True}

ghc_mingw_transition = transition(
    implementation = _impl,
    inputs = [],
    outputs = [_FLAG],
)
