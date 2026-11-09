# PHP

PHP test fixtures. They live in the `rules_dx_php` module, next to `rules_dx`.
The rules, the toolchain extension, the Composer extension, the lint and
typecheck aspects and the Gazelle language stay in `rules_dx`. The module
registers the PHP toolchains, so a workspace that does not use PHP does not
fetch them.
