"""Workflow host and execution capability matrix for #1455."""

WORKFLOW_CAPABILITY_HOSTS = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "windows_arm64",
    "windows_x86_64",
]

WORKFLOW_CAPABILITIES = [
    "android.acquire",
    "android.apk",
    "android.native",
    "apple.app",
    "apple.lib",
    "consumer.shared",
    "js.consume",
    "native.run",
    "wasi.exec",
    "wasm.bindgen",
    "wasm.browser",
    "wasm.node",
    "web.bundle",
    "web.serve",
]

WORKFLOW_CAPABILITY_STATES = [
    "implemented",
    "planned",
    "qualified",
    "unsupported",
    "verification-pending",
]

WORKFLOW_CAPABILITY_REASONS = [
    "needs-android-sdk",
    "needs-host-run",
    "needs-macos-xcode",
]

WORKFLOW_CAPABILITIES_TABLE = [
    {
        "capability": "android.acquire",
        "evidence": [],
        "host": "linux_arm64",
        "id": "android.acquire.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.acquire",
        "evidence": ["//android/sdk:metadata"],
        "host": "linux_x86_64",
        "id": "android.acquire.linux_x86_64",
        "reason": "",
        "state": "implemented",
    },
    {
        "capability": "android.acquire",
        "evidence": [],
        "host": "macos_arm64",
        "id": "android.acquire.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.acquire",
        "evidence": [],
        "host": "windows_arm64",
        "id": "android.acquire.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.acquire",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "android.acquire.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.apk",
        "evidence": [],
        "host": "linux_arm64",
        "id": "android.apk.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.apk",
        "evidence": [
            "//android/packaging:bad_abi_test",
            "//android/tests/fixtures/rust_apk:app_rust_test",
        ],
        "host": "linux_x86_64",
        "id": "android.apk.linux_x86_64",
        "reason": "",
        "state": "implemented",
    },
    {
        "capability": "android.apk",
        "evidence": [],
        "host": "macos_arm64",
        "id": "android.apk.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.apk",
        "evidence": [],
        "host": "windows_arm64",
        "id": "android.apk.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.apk",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "android.apk.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.native",
        "evidence": [],
        "host": "linux_arm64",
        "id": "android.native.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.native",
        "evidence": [
            "//android/platforms:metadata",
            "//android/tests/fixtures/native_lib:native_lib_host_test",
        ],
        "host": "linux_x86_64",
        "id": "android.native.linux_x86_64",
        "reason": "",
        "state": "implemented",
    },
    {
        "capability": "android.native",
        "evidence": [],
        "host": "macos_arm64",
        "id": "android.native.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.native",
        "evidence": [],
        "host": "windows_arm64",
        "id": "android.native.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "android.native",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "android.native.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.app",
        "evidence": [],
        "host": "linux_arm64",
        "id": "apple.app.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.app",
        "evidence": [
            "//apple/ios:app_metadata",
            "//apple/ios:export_on_check_test",
        ],
        "host": "linux_x86_64",
        "id": "apple.app.linux_x86_64",
        "reason": "",
        "state": "implemented",
    },
    {
        "capability": "apple.app",
        "evidence": [],
        "host": "macos_arm64",
        "id": "apple.app.macos_arm64",
        "reason": "needs-macos-xcode",
        "state": "verification-pending",
    },
    {
        "capability": "apple.app",
        "evidence": [],
        "host": "windows_arm64",
        "id": "apple.app.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.app",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "apple.app.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.lib",
        "evidence": [],
        "host": "linux_arm64",
        "id": "apple.lib.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.lib",
        "evidence": [
            "//apple/ios:bad_environment_test",
            "//apple/ios:device_only_test",
        ],
        "host": "linux_x86_64",
        "id": "apple.lib.linux_x86_64",
        "reason": "",
        "state": "implemented",
    },
    {
        "capability": "apple.lib",
        "evidence": [],
        "host": "macos_arm64",
        "id": "apple.lib.macos_arm64",
        "reason": "needs-macos-xcode",
        "state": "verification-pending",
    },
    {
        "capability": "apple.lib",
        "evidence": [],
        "host": "windows_arm64",
        "id": "apple.lib.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "apple.lib",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "apple.lib.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "consumer.shared",
        "evidence": [],
        "host": "linux_arm64",
        "id": "consumer.shared.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "consumer.shared",
        "evidence": [
            "//examples/shared-rust:consumer_browser_test",
            "//examples/shared-rust:consumer_native_test",
        ],
        "host": "linux_x86_64",
        "id": "consumer.shared.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "consumer.shared",
        "evidence": [],
        "host": "macos_arm64",
        "id": "consumer.shared.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "consumer.shared",
        "evidence": [],
        "host": "windows_arm64",
        "id": "consumer.shared.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "consumer.shared",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "consumer.shared.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "js.consume",
        "evidence": [],
        "host": "linux_arm64",
        "id": "js.consume.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "js.consume",
        "evidence": [
            "//rust/tests/fixtures/wasm_bindgen:wasm_js_parity_test",
            "//rust/tests/fixtures/wasm_bindgen:wasm_ts_consumer_upstream_typecheck_test",
        ],
        "host": "linux_x86_64",
        "id": "js.consume.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "js.consume",
        "evidence": [],
        "host": "macos_arm64",
        "id": "js.consume.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "js.consume",
        "evidence": [],
        "host": "windows_arm64",
        "id": "js.consume.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "js.consume",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "js.consume.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "native.run",
        "evidence": [],
        "host": "linux_arm64",
        "id": "native.run.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "native.run",
        "evidence": [
            "//examples/shared-rust:asset_test",
            "//examples/shared-rust:core_test",
        ],
        "host": "linux_x86_64",
        "id": "native.run.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "native.run",
        "evidence": [],
        "host": "macos_arm64",
        "id": "native.run.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "native.run",
        "evidence": [],
        "host": "windows_arm64",
        "id": "native.run.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "native.run",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "native.run.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasi.exec",
        "evidence": [],
        "host": "linux_arm64",
        "id": "wasi.exec.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasi.exec",
        "evidence": [
            "//rust/tests/fixtures/consumer_wasi:custom_runtime",
            "//rust/tests/fixtures/consumer_wasi:declared_capabilities",
            "//rust/tests/fixtures/consumer_wasi:default_run",
        ],
        "host": "linux_x86_64",
        "id": "wasi.exec.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "wasi.exec",
        "evidence": [],
        "host": "macos_arm64",
        "id": "wasi.exec.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasi.exec",
        "evidence": [],
        "host": "windows_arm64",
        "id": "wasi.exec.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasi.exec",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "wasi.exec.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.bindgen",
        "evidence": [],
        "host": "linux_arm64",
        "id": "wasm.bindgen.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.bindgen",
        "evidence": [
            "//rust/tests/fixtures/consumer_wasm:consumer_wasm_output_test",
            "//rust/tests/fixtures/wasm_bindgen:wasm_hello_bundle_test",
        ],
        "host": "linux_x86_64",
        "id": "wasm.bindgen.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "wasm.bindgen",
        "evidence": [],
        "host": "macos_arm64",
        "id": "wasm.bindgen.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.bindgen",
        "evidence": [],
        "host": "windows_arm64",
        "id": "wasm.bindgen.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.bindgen",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "wasm.bindgen.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.browser",
        "evidence": [],
        "host": "linux_arm64",
        "id": "wasm.browser.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.browser",
        "evidence": [
            "//examples/shared-rust:browser_test",
            "//javascript/tests/fixtures/web_app:web_browser_test",
            "//rust/tests/fixtures/wasm_bindgen:wasm_hello_browser_test",
        ],
        "host": "linux_x86_64",
        "id": "wasm.browser.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "wasm.browser",
        "evidence": [],
        "host": "macos_arm64",
        "id": "wasm.browser.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.browser",
        "evidence": [],
        "host": "windows_arm64",
        "id": "wasm.browser.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.browser",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "wasm.browser.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.node",
        "evidence": [],
        "host": "linux_arm64",
        "id": "wasm.node.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.node",
        "evidence": ["//rust/tests/fixtures/wasm_bindgen:wasm_node_test"],
        "host": "linux_x86_64",
        "id": "wasm.node.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "wasm.node",
        "evidence": [],
        "host": "macos_arm64",
        "id": "wasm.node.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.node",
        "evidence": [],
        "host": "windows_arm64",
        "id": "wasm.node.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "wasm.node",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "wasm.node.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.bundle",
        "evidence": [],
        "host": "linux_arm64",
        "id": "web.bundle.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.bundle",
        "evidence": [
            "//javascript/tests/fixtures/web_app:bundler_executable_test",
            "//javascript/tests/fixtures/web_app:web_bundle_test",
        ],
        "host": "linux_x86_64",
        "id": "web.bundle.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "web.bundle",
        "evidence": [],
        "host": "macos_arm64",
        "id": "web.bundle.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.bundle",
        "evidence": [],
        "host": "windows_arm64",
        "id": "web.bundle.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.bundle",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "web.bundle.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.serve",
        "evidence": [],
        "host": "linux_arm64",
        "id": "web.serve.linux_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.serve",
        "evidence": [
            "//examples/shared-rust:browser_test",
            "//javascript/tests/fixtures/web_app:web_server_test",
        ],
        "host": "linux_x86_64",
        "id": "web.serve.linux_x86_64",
        "reason": "",
        "state": "qualified",
    },
    {
        "capability": "web.serve",
        "evidence": [],
        "host": "macos_arm64",
        "id": "web.serve.macos_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.serve",
        "evidence": [],
        "host": "windows_arm64",
        "id": "web.serve.windows_arm64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
    {
        "capability": "web.serve",
        "evidence": [],
        "host": "windows_x86_64",
        "id": "web.serve.windows_x86_64",
        "reason": "needs-host-run",
        "state": "verification-pending",
    },
]

def _is_label(text):
    """Reports whether a text names a Bazel package or external label."""
    return text.startswith("//") or text.startswith("@")

def workflow_record_error(record):
    """Validates one capability row against the matrix schema."""
    for key in ["capability", "evidence", "host", "id", "reason", "state"]:
        if key not in record:
            return "workflow: record is missing field '" + key + "'"
    capability = record["capability"]
    host = record["host"]
    state = record["state"]
    if capability not in WORKFLOW_CAPABILITIES:
        return "workflow: unknown capability '" + capability + "'"
    if host not in WORKFLOW_CAPABILITY_HOSTS:
        return "workflow: unknown host '" + host + "'"
    if state not in WORKFLOW_CAPABILITY_STATES:
        return "workflow: row '" + record["id"] + "' needs state 'implemented', 'planned', 'qualified', 'unsupported' or 'verification-pending', got '" + str(state) + "'"
    if record["id"] != capability + "." + host:
        return "workflow: row id '" + record["id"] + "' must be '" + capability + "." + host + "'"
    evidence = record["evidence"]
    if type(evidence) != "list":
        return "workflow: row '" + record["id"] + "' needs an evidence list"
    for target in evidence:
        if not _is_label(target):
            return "workflow: row '" + record["id"] + "' names non-label evidence '" + str(target) + "'"
    reason = record["reason"]
    if state == "qualified" or state == "implemented":
        if len(evidence) == 0:
            return "workflow: row '" + record["id"] + "' is '" + state + "' but names no evidence"
        if reason != "":
            return "workflow: row '" + record["id"] + "' is '" + state + "' but names reason '" + reason + "'"
    if state == "verification-pending" or state == "unsupported":
        if len(evidence) != 0:
            return "workflow: row '" + record["id"] + "' is '" + state + "' but names evidence"
        if reason not in WORKFLOW_CAPABILITY_REASONS:
            return "workflow: row '" + record["id"] + "' needs reason 'needs-android-sdk', 'needs-host-run' or 'needs-macos-xcode', got '" + str(reason) + "'"
    if state == "planned":
        if len(evidence) != 0 or reason != "":
            return "workflow: row '" + record["id"] + "' is 'planned' but carries evidence or reason"
    return ""

def workflow_table_error():
    """Validates every matrix row, reporting the first failure."""
    for record in WORKFLOW_CAPABILITIES_TABLE:
        err = workflow_record_error(record)
        if err != "":
            return err
    return ""

def _ids():
    """Returns the matrix ids in table order."""
    return [record["id"] for record in WORKFLOW_CAPABILITIES_TABLE]

def workflow_order_error():
    """Reports matrix ids that break sorted order."""
    ids = _ids()
    if ids != sorted(ids):
        for i in range(len(ids) - 1):
            if ids[i] > ids[i + 1]:
                return "workflow: row '" + ids[i] + "' sorts after '" + ids[i + 1] + "'"
    return ""

def workflow_coverage_error():
    """Reports capability and host combinations missing from the matrix."""
    seen = {}
    for record in WORKFLOW_CAPABILITIES_TABLE:
        seen[record["capability"] + "." + record["host"]] = True
    missing = []
    for capability in WORKFLOW_CAPABILITIES:
        for host in WORKFLOW_CAPABILITY_HOSTS:
            if capability + "." + host not in seen:
                missing.append(capability + "." + host)
    if len(missing) != 0:
        return "workflow: matrix is missing rows: " + ", ".join(missing)
    extra = [id for id in _ids() if id not in [c + "." + h for c in WORKFLOW_CAPABILITIES for h in WORKFLOW_CAPABILITY_HOSTS]]
    if len(extra) != 0:
        return "workflow: matrix names rows outside the capability set: " + ", ".join(sorted(extra))
    return ""

def _row(id):
    """Returns the matrix row with this id, or None."""
    for record in WORKFLOW_CAPABILITIES_TABLE:
        if record["id"] == id:
            return record
    return None

def workflow_state(capability, host):
    """Returns the matrix state for one combination."""
    row = _row(capability + "." + host)
    if row == None:
        return ""
    return row["state"]

def workflow_evidence(capability, host):
    """Returns the matrix evidence for one combination."""
    row = _row(capability + "." + host)
    if row == None:
        return []
    return row["evidence"]

def qualified_without_evidence():
    """Returns the qualified rows that name no evidence."""
    return [record["id"] for record in WORKFLOW_CAPABILITIES_TABLE if record["state"] == "qualified" and len(record["evidence"]) == 0]

def pending_without_reason():
    """Returns the pending rows that name no usable reason."""
    return [record["id"] for record in WORKFLOW_CAPABILITIES_TABLE if record["state"] == "verification-pending" and record["reason"] not in WORKFLOW_CAPABILITY_REASONS]
