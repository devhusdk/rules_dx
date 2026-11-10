-module(dx_runtime_probe).

-export([otp_version/0]).

-spec otp_version() -> string().
otp_version() ->
    Release = erlang:system_info(otp_release),
    File = filename:join([code:root_dir(), "releases", Release, "OTP_VERSION"]),
    {ok, Version} = file:read_file(File),
    string:trim(binary_to_list(Version)).
