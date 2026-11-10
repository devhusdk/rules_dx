//! The root MODULE.bazel.lock stays within the committed size and extension budget. This is a
//! whole-repo number; the external repo count needs a fetched consumer.

use dx_nested::footprint::{check_lock_budget, measure_lock};
use dx_testing::{read_runfiles, read_text, resolve_runfiles};

#[test]
fn the_root_lock_is_within_the_budget() {
    let budget_path = std::env::var("DX_PYTHON_BUDGET")
        .unwrap_or_else(|_| panic!("DX_PYTHON_BUDGET must name the budget"));
    let budget: serde_json::Value =
        serde_json::from_str(&read_text(&resolve_runfiles(&budget_path))).expect("budget JSON");
    let measured = measure_lock(&read_runfiles("MODULE.bazel.lock")).expect("measure the lock");
    if let Err(error) = check_lock_budget(&measured, &budget) {
        panic!("{error}");
    }
}
