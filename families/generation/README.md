# Code generation

Avro, Cap'n Proto, FlatBuffers, Thrift and OpenAPI client tests and fixtures. They live in the
`rules_dx_generation` module, next to `rules_dx`. The rules, the
`generation/` shard helpers, protobuf and the generator archives stay in
`rules_dx`, because its quality tools, Gazelle extensions and CLI use them.
A workspace that only uses the code generation rules needs nothing more than
`rules_dx`.

## Protobuf docs

`protobuf_doc_shard` in `@rules_dx//tools/docs/protobuf:defs.bzl` documents one
`proto_library`. It runs the protoc of the registered proto toolchain with
`--include_source_info` and reads the comments above each message, field, enum,
enum value, service and rpc from the descriptor set. 
## Avro docs

`avro_doc_shard` in `@rules_dx//tools/docs/avro:defs.bzl` documents one
`avro_library`. The `doc` attribute of every record, field, enum, fixed type and
protocol message is read from the `.avsc` files directly. An `.avdl` file is
first converted with `idl2schemata` of the pinned avro-tools (`//config:tool_avro`),
which runs on the hermetic JDK, and then read the same way.
