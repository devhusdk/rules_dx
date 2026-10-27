# Namespace resolution of `using` directives

Verifies that a `using` resolves to the local library that declares the
namespace, not to a source file with the same stem. `app` uses `Acme.Lib`,
`A.Util` and `Solo`, so it depends on `//lib`, `//a/util` and `//shout`,
the library that declares `namespace Solo`. The `solo` library holds
`Solo.cs` but declares `namespace Other`, so the file name no longer
matches. `tool` has `using static B.Util.Y;`, which resolves through the
namespace-qualified file stem to `//b/util`.
