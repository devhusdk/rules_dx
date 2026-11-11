const std = @import("std");
const conformance = @import("conformance");

pub fn main(init: std.process.Init) !void {
    var buf: [32]u8 = undefined;
    var out_buf: [64]u8 = undefined;
    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);
    try out.interface.print("{s}\n", .{try conformance.greet(&buf, "world")});
    try out.interface.flush();
}
