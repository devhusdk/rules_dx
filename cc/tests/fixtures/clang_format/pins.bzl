"""Clang-format check plus fix wiring."""

CLANG_FORMAT_VERSION = "hermetic-llvm v0.8.23 (LLVM 23.1.0)"

CLANG_FORMAT_ARTIFACT = "authoritative hermetic-llvm LLVM tool targets (clang-format), no separate acquisition"

CLANG_FORMAT_CHECK = "clang-format --dry-run --Werror check with violation diagnostics on stderr (exit 0 clean, exit 1 with file:line:column violations when dirty)"
CLANG_FORMAT_FIX = "clang-format -i in-place rewrite (re-read on exit 0, keep input otherwise)"
CLANG_FORMAT_CONFIG_POLICY = "checked-in .clang-format required to change policy; upstream built-in defaults without config, native interpretation with config"

CC_FIXTURE_HELLO = "//cc/tests/fixtures/hello:hello_test"

CLANG_FORMAT_REJECTED = "ambient .clang-format discovery rejected; auto-supplied --style preset rejected"
