"""Unit tests for the workflow capability matrix."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":workflow_capabilities.bzl", "pending_without_reason", "qualified_without_evidence", "workflow_coverage_error", "workflow_evidence", "workflow_order_error", "workflow_record_error", "workflow_state", "workflow_table_error")

def _good_row():
    """Returns a minimal valid matrix row."""
    return {
        "capability": "wasm.node",
        "evidence": ["//rust/tests/fixtures/wasm_bindgen:wasm_node_test"],
        "host": "linux_x86_64",
        "id": "wasm.node.linux_x86_64",
        "reason": "",
        "state": "qualified",
    }

def workflow_capability_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "matrix schema validates",
                workflow_table_error(),
                "",
            ),
            expect_equal(
                "matrix ids stay sorted",
                workflow_order_error(),
                "",
            ),
            expect_equal(
                "matrix covers every capability and host",
                workflow_coverage_error(),
                "",
            ),
            expect_equal(
                "qualified rows carry evidence",
                qualified_without_evidence(),
                [],
            ),
            expect_equal(
                "pending rows carry reasons",
                pending_without_reason(),
                [],
            ),
            expect_equal(
                "linux browser execution stays qualified",
                workflow_state("wasm.browser", "linux_x86_64"),
                "qualified",
            ),
            expect_equal(
                "linux browser evidence keeps its executed targets",
                workflow_evidence("wasm.browser", "linux_x86_64"),
                [
                    "//examples/shared-rust:browser_test",
                    "//javascript/tests/fixtures/web_app:web_browser_test",
                    "//rust/tests/fixtures/wasm_bindgen:wasm_hello_browser_test",
                ],
            ),
            expect_equal(
                "linux android rows stay implemented without device evidence",
                [workflow_state("android.native", "linux_x86_64"), workflow_state("android.apk", "linux_x86_64"), workflow_state("android.acquire", "linux_x86_64")],
                ["implemented", "implemented", "implemented"],
            ),
            expect_equal(
                "linux apple rows stay implemented without macos execution",
                [workflow_state("apple.lib", "linux_x86_64"), workflow_state("apple.app", "linux_x86_64")],
                ["implemented", "implemented"],
            ),
            expect_equal(
                "macos apple rows wait on xcode execution",
                workflow_record_error({"capability": "apple.lib", "evidence": [], "host": "macos_arm64", "id": "apple.lib.macos_arm64", "reason": "needs-macos-xcode", "state": "verification-pending"}),
                "",
            ),
            expect_equal(
                "bad state is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": [], "host": "linux_x86_64", "id": "wasm.node.linux_x86_64", "reason": "", "state": "maybe"}),
                "workflow: row 'wasm.node.linux_x86_64' needs state 'implemented', 'planned', 'qualified', 'unsupported' or 'verification-pending', got 'maybe'",
            ),
            expect_equal(
                "unknown capability is rejected",
                workflow_record_error({"capability": "wasm.hologram", "evidence": [], "host": "linux_x86_64", "id": "wasm.hologram.linux_x86_64", "reason": "", "state": "planned"}),
                "workflow: unknown capability 'wasm.hologram'",
            ),
            expect_equal(
                "unknown host is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": [], "host": "linux_mips", "id": "wasm.node.linux_mips", "reason": "", "state": "planned"}),
                "workflow: unknown host 'linux_mips'",
            ),
            expect_equal(
                "mismatched id is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": [], "host": "linux_x86_64", "id": "wasm.node.linux_arm64", "reason": "", "state": "planned"}),
                "workflow: row id 'wasm.node.linux_arm64' must be 'wasm.node.linux_x86_64'",
            ),
            expect_equal(
                "qualified row without evidence is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": [], "host": "linux_x86_64", "id": "wasm.node.linux_x86_64", "reason": "", "state": "qualified"}),
                "workflow: row 'wasm.node.linux_x86_64' is 'qualified' but names no evidence",
            ),
            expect_equal(
                "pending row without reason is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": [], "host": "windows_x86_64", "id": "wasm.node.windows_x86_64", "reason": "", "state": "verification-pending"}),
                "workflow: row 'wasm.node.windows_x86_64' needs reason 'needs-android-sdk', 'needs-host-run' or 'needs-macos-xcode', got ''",
            ),
            expect_equal(
                "non-label evidence is rejected",
                workflow_record_error({"capability": "wasm.node", "evidence": ["wasm_node_test"], "host": "linux_x86_64", "id": "wasm.node.linux_x86_64", "reason": "", "state": "qualified"}),
                "workflow: row 'wasm.node.linux_x86_64' names non-label evidence 'wasm_node_test'",
            ),
            expect_equal(
                "missing key is rejected",
                workflow_record_error({"capability": "wasm.node", "host": "linux_x86_64", "id": "wasm.node.linux_x86_64", "reason": "", "state": "planned"}),
                "workflow: record is missing field 'evidence'",
            ),
            expect_equal(
                "valid row passes",
                workflow_record_error(_good_row()),
                "",
            ),
        ],
    )
