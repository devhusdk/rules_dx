import dx_pins
import dx_runtime_probe
import gleam/string

pub fn greets_test() {
  assert dx_runtime_probe.greet("Bazel") == "Hello, Bazel!"
}

pub fn runs_on_erlang_test() {
  assert dx_runtime_probe.runtime() == "erlang"
}

pub fn runs_on_the_pinned_otp_test() {
  assert dx_runtime_probe.version() == dx_pins.otp_release
}

pub fn runs_from_the_declared_runtime_test() {
  assert string.contains(dx_runtime_probe.root_dir(), "dx_otp_")
}
