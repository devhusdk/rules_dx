# NIF App

Sources under `c_src` become one `erlang_nif` in the app package, named after
the `erlang:load_nif` path. No `c_src/BUILD.bazel` is written.

`fast_lexer`, generated from `src/fast_lexer.xrl`, counts as a defined module.
