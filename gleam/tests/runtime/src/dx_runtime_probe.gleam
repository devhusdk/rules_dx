pub fn greet(name: String) -> String {
  "Hello, " <> name <> "!"
}

@external(erlang, "dx_runtime_probe_ffi", "runtime")
@external(javascript, "./dx_runtime_probe_ffi.mjs", "runtime")
pub fn runtime() -> String

@external(erlang, "dx_runtime_probe_ffi", "version")
@external(javascript, "./dx_runtime_probe_ffi.mjs", "version")
pub fn version() -> String

@external(erlang, "dx_runtime_probe_ffi", "root_dir")
@external(javascript, "./dx_runtime_probe_ffi.mjs", "root_dir")
pub fn root_dir() -> String

@external(erlang, "dx_runtime_probe_ffi", "file_exists")
@external(javascript, "./dx_runtime_probe_ffi.mjs", "file_exists")
pub fn file_exists(path: String) -> Bool
