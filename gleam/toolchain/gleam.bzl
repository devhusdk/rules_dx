"""Gleam Erlang toolchain on the shared pinned OTP tree."""

load("@rules_gleam//gleam_tools:toolchain.bzl", "GleamErlangToolsInfo")

OTP_TOOLS = ["erl", "erlc", "escript"]

def otp_tool_paths(root):
    """Returns the OTP launcher path for each tool Gleam runs."""
    return {tool: root + "/bin/" + tool for tool in OTP_TOOLS}

def _dx_gleam_erlang_toolchain_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    files = otp.tree.to_list()
    wanted = otp_tool_paths(otp.version_file.dirname)
    found = {tool: f for f in files for tool, path in wanted.items() if f.path == path}
    for tool in OTP_TOOLS:
        if tool not in found:
            fail("OTP tree at " + otp.version_file.dirname + " has no bin/" + tool)
    return [platform_common.ToolchainInfo(
        gleamerlangtools = GleamErlangToolsInfo(
            escript = found["escript"],
            escript_label = None,
            erl = found["erl"],
            erlc = found["erlc"],
            otp = files,
        ),
    )]

dx_gleam_erlang_toolchain = rule(
    implementation = _dx_gleam_erlang_toolchain_impl,
    doc = "Exposes the resolved OTP erl, erlc and escript to rules_gleam.",
    toolchains = ["@rules_erlang//tools:toolchain_type"],
)
