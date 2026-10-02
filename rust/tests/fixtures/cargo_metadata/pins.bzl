"""Cargo metadata pins."""

FEATURES_SHAPE = "nonempty required-features stays opt-in with kept testonly"
FEATURES_KEPT_ROUTE = "keep a handwritten testonly target selecting them"
FEATURES_NO_UNCONDITIONAL = "does not enable features or emit the target unconditionally"
FEATURES_NO_PRIVATE_RESOLVER = "no private rules_rs data dependency and no project-owned Cargo feature resolver"
FEATURES_NARROW_EXPORT = "until upstream exposes exact first-party target admissibility"
FEATURES_CRATE_FEATURES = "explicit crate_features"

BUILD_SCRIPT_SHAPE = "conventional build.rs plus explicit package.build plus build=false authoritative"
BUILD_SCRIPT_VERSION = "package version feeds script version plus single-version checks"
BUILD_SCRIPT_SCOPES = "script sees only [build-dependencies]"
BUILD_SCRIPT_DEFAULTS = "use_cc_toolchain True plus use_default_shell_env False plus emit_warnings True"
BUILD_SCRIPT_EXEC = "script runs later as a Bazel execution action, never during generation"
BUILD_SCRIPT_NO_PROTOCOL = "generation does not recreate Cargo's protocol"
BUILD_SCRIPT_NARROW_EXPORT = "BuildScriptInfo outputs stay narrow upstream metadata exports"

TARGET_KINDS_SHAPE = "ordinary libraries plus binaries plus tests plus proc macros plus sole cdylib plus sole staticlib"
TARGET_KINDS_PUBLIC_MAPPING = "only through the stable fixture-proven public upstream mapping"
TARGET_KINDS_REJECTED = "dylib plus multiple crate types plus unknown kinds fail with kept handwritten-target route"
TARGET_KINDS_NO_COERCION = "generation does not coerce kinds or duplicate the crate root"
EXAMPLE_BENCH_SHAPE = "only explicitly declared ordinary-binary targets with affixed names"

OWNERSHIP_SHAPE = "authoritative crate root owns its root and every recursively loaded module"
OWNERSHIP_TEST_ROOTS = "integration-test roots separately own their trees"
OWNERSHIP_ONE_OWNER = "one source may have only one owner"
OWNERSHIP_SIBLING_LIB = "bins plus tests plus examples plus benches link the same-package library automatically"
OWNERSHIP_MIRROR = "declared first-party path dependencies mirror without detection evidence"
OWNERSHIP_VISIBILITY = "emitted library flavors stay //visibility:public while bins plus tests plus scripts stay private"

PUBLIC_SHAPE = "checked-in Cargo declarations parsed in the first-party extension"
PUBLIC_INDEX = "path crates resolved through Gazelle's local rule index"
PUBLIC_MACROS = "exact imported external names emitted through the public @crates//:crates.bzl crate_deps and aliases macros"
PUBLIC_PACKAGE_NAME = "parent Bazel directory joined with the Cargo package name"
PUBLIC_NEVER_READS = "never reads Cargo.Bazel.lock plus cargo-bazel.json plus private dependency maps"
PUBLIC_NO_PRIVATE_GRAPH = "without private serialized dependency-graph access"

REJECTED_ALTERNATIVES = [
    "ad-hoc metadata",
    "unconditional required-features target",
    "kind coercion",
    "duplicate root ownership",
    "private serialized dependency-graph access",
    "Cargo.Bazel.lock read",
    "cargo-bazel.json read",
    "private dependency maps read",
]

CARGO_METADATA_FIXTURE_MANIFEST = "//rust/tests/fixtures/cargo_metadata:Cargo.toml"
CARGO_METADATA_LIVE_HELLO = "//rust/tests/fixtures/hello:hello"
CARGO_METADATA_LIVE_CC_OPTOUT = "//rust/tests/fixtures/cc_optout:cc_optout"
CARGO_METADATA_TESTDATA_APP = "gazelle/rust/testdata/cargo/crates/app"
CARGO_METADATA_TESTDATA_SHAPES = "gazelle/rust/testdata/cargo/crates/shapes"
