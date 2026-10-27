# Compile External

A real `scalac` compiles each package with only its generated deps, and fails once any one of them
is gone. `scala.xml` and the grouped `org.scalactic` selector resolve through their mappings, and
`scala.collection.mutable` stays in the standard library.
