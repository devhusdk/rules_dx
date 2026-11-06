"""Maven lock wiring pins."""

RULES_JVM_EXTERNAL_VERSION = "7.1"

MAVEN_LOCK_FILE = "//third_party/jvm:maven_install.json"
MAVEN_FAIL_IF_REPIN_REQUIRED = True

MAVEN_REPIN = "REPIN=1 bazel run @maven//:pin"

MAVEN_ARTIFACTS = [
    "junit:junit:4.13.2",
    "org.junit.jupiter:junit-jupiter-api:6.1.3",
    "org.junit.jupiter:junit-jupiter-engine:6.1.3",
    "org.junit.jupiter:junit-jupiter-params:6.1.3",
    "org.junit.platform:junit-platform-commons:6.1.3",
    "org.junit.platform:junit-platform-engine:6.1.3",
    "org.junit.platform:junit-platform-launcher:6.1.3",
    "org.junit.platform:junit-platform-console:6.1.3",
    "org.junit.vintage:junit-vintage-engine:6.1.3",
    "org.openjdk.jmh:jmh-core:1.37",
    "org.openjdk.jmh:jmh-generator-annprocess:1.37",
    "org.scalactic:scalactic_2.13:3.2.20",
    "org.scalatest:scalatest_2.13:3.2.20",
    "org.scalatest:scalatest-compatible:3.2.20",
    "org.scalatest:scalatest-core_2.13:3.2.20",
    "org.scalatest:scalatest-diagrams_2.13:3.2.20",
    "org.scalatest:scalatest-featurespec_2.13:3.2.20",
    "org.scalatest:scalatest-flatspec_2.13:3.2.20",
    "org.scalatest:scalatest-freespec_2.13:3.2.20",
    "org.scalatest:scalatest-funspec_2.13:3.2.20",
    "org.scalatest:scalatest-funsuite_2.13:3.2.20",
    "org.scalatest:scalatest-matchers-core_2.13:3.2.20",
    "org.scalatest:scalatest-mustmatchers_2.13:3.2.20",
    "org.scalatest:scalatest-propspec_2.13:3.2.20",
    "org.scalatest:scalatest-refspec_2.13:3.2.20",
    "org.scalatest:scalatest-shouldmatchers_2.13:3.2.20",
    "org.scalatest:scalatest-wordspec_2.13:3.2.20",
]

MAVEN_REPOSITORIES = [
    "https://maven-central.storage-download.googleapis.com/maven2",
    "https://repo1.maven.org/maven2",
]

MAVEN_FIXTURE_JAVA_JUPITER = "@rules_dx_jvm//java/tests/fixtures/junit:hello_jupiter_test"
MAVEN_FIXTURE_KOTLIN_JUPITER = "@rules_dx_jvm//kotlin/tests/fixtures/junit:hello_jupiter_test"
MAVEN_FIXTURE_JAVA_SEED = "@rules_dx_jvm//java/tests/fixtures/hello:hello_test"
MAVEN_FIXTURE_KOTLIN_SEED = "@rules_dx_jvm//kotlin/tests/fixtures/hello:hello_test"
MAVEN_FIXTURE_SCALA_SEED = "@rules_dx_jvm//scala/tests/fixtures/hello:hello_test"

MAVEN_REJECTED = "non-fail-closed rejected: no lock without fail_if_repin_required, no floating coordinates, no hand-edited lock"

MAVEN_CURRENCY_RECHECK = "2026-09-22"
