-module(diagnostics_SUITE).

-include_lib("stdlib/include/assert.hrl").

-export([all/0]).
-export([runs_the_pinned_runtime/1]).

all() ->
    [runs_the_pinned_runtime].

runs_the_pinned_runtime(_Config) ->
    ?assertEqual(os:getenv("DX_OTP_VERSION"), dx_runtime_probe:otp_version()).
