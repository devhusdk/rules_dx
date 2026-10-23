import dx_pins
import dx_runtime_probe
import gleam/string

pub fn greets_test() {
  assert dx_runtime_probe.greet("Bazel") == "Hello, Bazel!"
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
