# Declaration-only generation

Verifies that a declaration file with no same-stem source and no owning rule
gets a declaration-only `typescript_project`, that a regenerated declaration
rule is idempotent, that a stale declaration rule is removed once its `.d.ts`
is gone, and that a hand-written `declaration_srcs` on a source rule survives
and is indexed so importers resolve to it.
