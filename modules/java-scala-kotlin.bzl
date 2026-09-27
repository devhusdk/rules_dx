"""JVM foundation pins (Java, Kotlin, Scala managed route)."""

RULES_JAVA_VERSION = "9.7.0"
RULES_KOTLIN_VERSION = "2.4.10"
RULES_SCALA_VERSION = "7.3.0"
SCALA_VERSION = "2.13.18"
RULES_JVM_EXTERNAL_VERSION = "7.1"

MAVEN_LOCK = "//third_party/jvm:maven_install.json"
MAVEN_REPIN = "REPIN=1 bazel run @maven//:pin"

JVM_TOOL_VERSIONS = {
    "google-java-format": "1.35.0",
    "checkstyle": "14.1.0",
    "pmd": "7.27.0",
    "spotbugs": "4.10.4",
    "ktfmt": "0.63",
    "ktlint": "1.8.0",
}

JAVA_MAJOR = "21"
JAVA_RUNTIME_VERSION = "remotejdk_" + JAVA_MAJOR
