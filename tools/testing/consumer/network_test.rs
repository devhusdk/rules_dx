use consumer_network::cold_start_test::fetches_only_its_cold_start_list;
use consumer_network::prebuilt_guard_test::compiles_no_rules_dx_tool;

#[test]
fn go_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("go");
}

#[test]
fn java_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("java");
}

#[test]
fn rust_fetches_only_its_cold_start_list() {
    fetches_only_its_cold_start_list("rust");
}

#[test]
fn go_guard_consumer_compiles_no_rules_dx_tool() {
    compiles_no_rules_dx_tool("DX_GO_GUARD_LANGS");
}

#[test]
fn java_guard_consumer_compiles_no_rules_dx_tool() {
    compiles_no_rules_dx_tool("DX_JAVA_GUARD_LANGS");
}
