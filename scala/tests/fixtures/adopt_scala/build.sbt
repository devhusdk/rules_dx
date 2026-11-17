// Foreign sbt layout marker: one pinned module dep (guava 32.0.1-jre,
// matching third_party/jvm/maven_install.json plus @maven hub). The Scala
// Gazelle extension reads only `.scala` sources; generation resolves the
// `com.google.common.collect.ImmutableList` import through the
// `maven_install.json` package list and never writes this file.
ThisBuild / scalaVersion := "2.13.18"
ThisBuild / organization := "example.com"
lazy val root = (project in file(".")).aggregate(greet, solo)
lazy val greet = project.settings(
  libraryDependencies += "com.google.guava" % "guava" % "32.0.1-jre"
)
lazy val solo = project
