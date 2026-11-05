# C# Proto Import

Verifies that a `.proto` file gets a `csharp_proto_library` next to its `proto_library` when
`rules_dx_dotnet` is a module dependency, and that a `using Api;` resolves to it.
