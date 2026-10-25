import dx_pins
import dx_runtime_probe
import gleam/string
import probe_helper

pub fn greets_test() {
  assert dx_runtime_probe.greet(probe_helper.audience()) == "Hello, Bazel!"
}

pub fn runs_on_node_test() {
  assert dx_runtime_probe.runtime() == "node"
}

pub fn runs_on_the_managed_node_test() {
  assert dx_runtime_probe.version() == dx_pins.node_version
}

pub fn runs_from_the_declared_runtime_test() {
  assert string.ends_with(dx_runtime_probe.root_dir(), "/bin/nodejs/bin/node")
}

pub fn reads_data_by_runfiles_path_test() {
  assert dx_runtime_probe.file_exists("gleam/tests/runtime/data.txt")
}
