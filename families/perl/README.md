# Perl

Perl test fixtures. They live in the `rules_dx_perl` module, next to
`rules_dx`. The rules, the CPAN extension, the lint and format aspects and the
Gazelle language stay in `rules_dx`, because its quality tools run on Perl.

## API docs

`dx docs` builds an API reference for every `perl_library` in scope. Declare a
`perl_doc_shard` from `@rules_dx//tools/docs/perl:defs.bzl` to name the package
or to set the shard by hand. It reads `package` and `sub` declarations and the
POD of the library's own `.pm` and `.pl` files with the pinned perl; `_`-prefixed
subs are left out. It does not run on `windows_arm64`.
