const std = @import("std");
const clap = @import("clap");

const params = clap.parseParamsComptime("-l, --lock Say hello.\n");

pub fn main(init: std.process.Init) !void {
    var out_buf: [64]u8 = undefined;
    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);
    try out.interface.print("hello {s}\n", .{params[0].names.long.?});
    try out.interface.flush();
}
