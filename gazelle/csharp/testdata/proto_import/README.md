# C# Proto Import

Verifies that a `.proto` file gets a `csharp_proto_library` next to its `proto_library` when
`rules_dx` is a module dependency and `paket.main` is a used repo, and that a `using Api;` resolves to it.
The C# library of a proto that imports another gets the C# library of the imported one as a dependency.
