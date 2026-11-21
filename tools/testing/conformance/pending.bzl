"""Suite tests that do not pass yet: one `<lang> <test> <platform|*>` line each."""

PENDING = """
android format_error_test windows_arm64
android format_test windows_arm64
android lint_error_test windows_arm64
android lint_test windows_arm64
ansible lock_test *
cuda cross_test linux_arm64
cuda cross_test windows_arm64
cuda cross_test windows_x86_64
elixir cross_test windows_x86_64
erlang cross_test windows_x86_64
fortran cross_test *
haskell format_error_test macos_arm64
haskell format_test macos_arm64
haskell lint_error_test macos_arm64
haskell lint_test macos_arm64
helm format_error_test *
helm format_test *
msix cross_test *
zig coverage_test windows_x86_64
"""
