"""JVM foundation pins (Java, Kotlin, Scala managed route)."""

load(":versions.bzl", _SCALA_VERSION = "SCALA_VERSION")

SCALA_VERSION = _SCALA_VERSION

JVM_TOOL_VERSIONS = {
    "scalafix": "0.14.9",
}

JAVA_MAJOR = "21"
JAVA_RUNTIME_VERSION = "remotejdk_" + JAVA_MAJOR
