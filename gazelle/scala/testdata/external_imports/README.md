# External Imports

A fully qualified external import resolves through an exact `# gazelle:resolve` mapping or a
mapping on its package, a wildcard `cats.effect._` resolves through the mapping on `cats.effect`,
and no stdlib prefix swallows either. Feature imports such as `scala.language.implicitConversions` and
`scala.util.chaining.scalaUtilChainingOps` stay stdlib and add no dependency.
