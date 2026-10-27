"""Cppcheck check-only wiring."""

CPPCHECK_VERSION = "2.21.0"

CPPCHECK_ARTIFACT = "checksummed upstream source archive built with the hermetic LLVM toolchain"

CPPCHECK_CHECK = "cppcheck --xml --xml-version=2 on stderr (exit 0 clean with empty errors, exit 1 with error elements when dirty)"
CPPCHECK_FIX = "check-only with the provisional sandbox-apply-and-diff fix flow"
CPPCHECK_CONFIG_POLICY = "upstream built-in default enablement without suppressions; --enable=all maxima never enabled"

CC_FIXTURE_HELLO = "//cc/tests/fixtures/hello:hello_test"

CPPCHECK_REJECTED = "--enable=all maxima rejected; text-parse fallback rejected; unparsable XML as clean rejected"
