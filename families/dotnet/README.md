# .NET

C# and F# rules and their tests. They live in the `rules_dx_dotnet` module, a
`bazel_dep` next to `rules_dx`. `rules_dx_dotnet` registers the .NET toolchain,
so a workspace with C# or F# targets adds `bazel_dep(name = "rules_dx_dotnet")`.
The pinned rules_dotnet, the SDK version and the NuGet pins stay in `rules_dx`.

- [C#](csharp/README.md)
- [F#](fsharp/README.md)
