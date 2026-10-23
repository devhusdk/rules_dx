-module(dx_runtime_probe_ffi).

-export([file_exists/1, root_dir/0, runtime/0, version/0]).

root_dir() -> list_to_binary(code:root_dir()).

runtime() -> <<"erlang">>.

version() -> list_to_binary(erlang:system_info(otp_release)).

file_exists(Path) -> filelib:is_regular(binary_to_list(Path)).
