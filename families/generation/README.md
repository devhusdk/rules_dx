# Code generation

Avro, Cap'n Proto, FlatBuffers, Thrift and OpenAPI client tests and fixtures. They live in the
`rules_dx_generation` module, next to `rules_dx`. The rules, the
`generation/` shard helpers, protobuf and the generator archives stay in
`rules_dx`, because its quality tools, Gazelle extensions and CLI use them.
A workspace that only uses the code generation rules needs nothing more than
`rules_dx`.
