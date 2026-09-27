"""ScalaTest 3.2.20 version pins."""

load("//modules:versions.bzl", _RULES_SCALA_VERSION = "RULES_SCALA_VERSION", _SCALA_VERSION = "SCALA_VERSION")

RULES_SCALA_VERSION = _RULES_SCALA_VERSION

SCALA_VERSION = _SCALA_VERSION

SCALATEST_VERSION = "3.2.20"
SCALACTIC_VERSION = "3.2.20"

SCALATEST_ARTIFACTS = [
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

SCALATEST_KIND = "scala_test"
SCALATEST_RUNNER = "ScalaTest"
SCALATEST_SUITE = "org.scalatest.flatspec.AnyFlatSpec"

SCALATEST_FIXTURE_LABEL = "//scala/tests/fixtures/hello:hello_test"
SCALATEST_FIXTURE_SUITE = "hello.HelloTest"
SCALATEST_MAVEN_LABEL = "@maven//:org_scalatest_scalatest_2_13"
SCALATEST_MAVEN_LOCK = "//third_party/jvm:maven_install.json"

SCALATEST_REJECTED = "unpinned runner rejected: no floating version or head"
