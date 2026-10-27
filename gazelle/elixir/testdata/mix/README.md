# Mix Project Generation

Each directory with a `mix.exs` gets one `elixir_app` from `lib/**/*.ex` and
one `ex_unit_test` from `test/**/*_test.exs` plus `test/test_helper.exs`. An
umbrella root gets no rule.

A module reference resolves to the `elixir_app` that defines the module.
Modules that ship with Elixir need no dependency. A resolve directive on a top
module, such as `Jason`, covers every module under it.

A handwritten `elixir_app` with the same `app_name` keeps its name and its
other attributes. Directories without a `mix.exs` stay as written.

Mix fills `deps/` and `_build/` next to a `mix.exs` with other projects. They
get no rules, even when they have their own `mix.exs`.
