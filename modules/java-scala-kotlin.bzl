"""JVM foundation pins (Java, Kotlin, Scala managed route)."""

load(":versions.bzl", _SCALA_VERSION = "SCALA_VERSION")

SCALA_VERSION = _SCALA_VERSION

JVM_TOOL_VERSIONS = {
    "google-java-format": "1.35.0",
    "checkstyle": "14.1.0",
    "pmd": "7.27.0",
    "scalafix": "0.14.9",
    "spotbugs": "4.10.4",
    "ktfmt": "0.63",
    "ktlint": "1.8.0",
}

JAVA_MAJOR = "21"
JAVA_RUNTIME_VERSION = "remotejdk_" + JAVA_MAJOR
