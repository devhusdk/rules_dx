"""Compilation-context fixture pins plus the target-coupling decision."""

CC_CONTEXT_LLVM_VERSION = "hermetic-llvm v0.8.19 (clang-tidy)"

CC_CONTEXT_LLVM_LABEL = "@llvm//tools:clang-tidy"

CC_CONTEXT_PRODUCER = "//cc/context:compile_db reading a bazel aquery --output=jsonproto record"

CC_CONTEXT_BUNDLE = "compile_commands.json plus the bound .clang-tidy in one output directory"

CC_CONTEXT_CONTRACT = "fails closed on a missing label, a missing required source, conflicting configurations, a relative execroot and an unreadable or empty policy"

CC_CONTEXT_CONFLICT = "two configurations of one source in one record are refused instead of resolved"

CC_CONTEXT_FIXTURE_LEGACY = "//cc/tests/fixtures/compilation_db:probe_legacy"

CC_CONTEXT_FIXTURE_PLAIN = "//cc/tests/fixtures/compilation_db:probe_plain"

CC_CONTEXT_FIXTURE_CONSUMER = "//cc/tests/fixtures/compilation_db:probe_consumer_toolchain"

CC_CONTEXT_FIXTURE_SOURCE = "cc/tests/fixtures/compilation_db/probe.c"

CC_CONTEXT_FIXTURE_GENERATED = "cc/tests/fixtures/compilation_db/generated/limits.h"

CC_CONTEXT_FIXTURE_POLICY = "cc/tests/fixtures/compilation_db/.clang-tidy"

CC_CONTEXT_REJECTED = "no-flags clean clang-tidy output rejected; aspect-inferred command lines rejected; unresolved configuration conflicts rejected"
