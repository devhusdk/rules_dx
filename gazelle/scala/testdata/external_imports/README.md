# External Imports

A fully qualified external import resolves through an exact `# gazelle:resolve` mapping and no
stdlib prefix swallows it. Feature imports such as `scala.language.implicitConversions` and
`scala.util.chaining.scalaUtilChainingOps` stay stdlib and add no dependency.
