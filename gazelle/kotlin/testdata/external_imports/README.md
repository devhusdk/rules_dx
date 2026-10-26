# External Imports

A fully qualified external import resolves through an exact `# gazelle:resolve` mapping or a
mapping on its package, a wildcard `kotlinx.coroutines.*` resolves through the mapping on
`kotlinx.coroutines`, and no stdlib prefix swallows either.
