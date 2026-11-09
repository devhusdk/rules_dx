# TypeScript Proto Import

Verifies that a `.proto` file gets a `typescript_proto_library` next to its `proto_library` when
`aspect_rules_ts` is a module dependency, and that an import of `../api/user_pb` resolves to it.
