// Package fsharp generates fsharp_library rules and orders their srcs.
//
// F# compiles files in list order, so the generator infers the order from
// each source: names that siblings declare at top level (types, exceptions,
// nested modules, file-level modules) and `open` paths. A .fsi file is
// grouped with the .fs file of the same stem and comes first. Cycles are
// refused with one named edge.
//
// Known gaps, where a source compiles in an order the inference does not see
// or where a resolve error remains:
//   - [<AutoOpen>] modules: names used through them are not traced to the file.
//   - Unqualified union cases and record fields: only type names are tracked.
//   - Extension members: `x.Ext()` does not tie to the file that adds Ext.
//   - `open` of the directory's own namespace or of a sibling nested module
//     orders correctly, but still reaches Resolve and fails as unresolved.
package fsharp
