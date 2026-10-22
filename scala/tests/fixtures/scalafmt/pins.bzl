"""Scalafmt check plus fix wiring."""

SCALAFMT_VERSION = "3.11.4"

SCALAFMT_ARTIFACT = "native binary"

SCALAFMT_CHECK = "scalafmt --list plus --config when hinted (exit 0 clean, exit 1 lists dirty files, other exits are errors)"
SCALAFMT_FIX = "scalafmt in-place rewrite (re-read on exit 0, keep input otherwise)"
SCALAFMT_CONFIG_POLICY = "bound .scalafmt.conf required; a missing config fails analysis"

SCALA_FIXTURE_HELLO = "//scala/tests/fixtures/hello:hello_test"

SCALAFMT_REJECTED = "IN_PLACE patching outside sandbox rejected; ambient .scalafmt.conf discovery rejected; auto-supplied preset rejected"
