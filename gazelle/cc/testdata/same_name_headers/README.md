# Same-Named Headers Resolve by Path

Two packages each own a `config.h`.
An include with a directory resolves to the header at that workspace path.
A bare include from a package resolves to that package's own header first.
